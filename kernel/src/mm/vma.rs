use alloc::collections::{BTreeMap, LinkedList};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::cmp::Ordering;
use core::default::Default;
use core::fmt::{Debug, Formatter};
use core::intrinsics::offset;
use core::mem::size_of;
use core::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};
use fatfs::{Read, Seek, SeekFrom, Write};
use log::set_max_level;
use crate::consts::PAGE_SIZE;

use crate::mm::addr::{OldAddr, Paddr, PageAlign, Vaddr};
use crate::mm::{alloc_one_page, get_kernel_pagetable};
use crate::utils::order2pages;
use crate::mm::mm::MmStruct;
use crate::mm::page::Page;
use crate::mm::pagetable::{PageTable, PTEFlags};
use crate::pre::{InnerAccess, ReadWriteOffUnsafe, ReadWriteSingleNoOff, ReadWriteSingleOff, ShowRdWrEx};
use crate::{println, SpinLock};
use crate::fs::inode::Inode;

bitflags! {
    pub struct VmFlags: usize {
        const VM_NONE = 0;
        const VM_READ = 1 << 0;
        const VM_WRITE = 1 << 1;
        const VM_EXEC = 1 << 2;
        const VM_USER = 1 << 3;
        const VM_SHARD = 1 << 4;
        const VM_ANON = 1 << 5;
        const VM_DIRTY = 1<< 6;
    }
}

bitflags! {
    pub struct MmapFlags: usize {
        const MAP_FILE = 0;
        const MAP_SHARED= 0x01;
        const MAP_PRIVATE = 0x02;
        const MAP_FIXED = 0x10;
        const MAP_ANONYMOUS = 0x20;
    }
}

bitflags! {
    pub struct MmapProt: usize {
        const PROT_READ = 0x1;
        const PROT_WRITE = 0x2;
        const PROT_EXEC = 0x4;
        const PROT_SEM = 0x8;
        const PROT_NONE = 0x0;
        const PROT_GROWSDOWN = 0x01000000;
        const PROT_GROWSUP = 0x02000000;
    }
}

// pub enum VmaType{
//     VmaNone,
//     VmaAnon,
//     VmaFile
// }

pub struct VMA{
    start_vaddr: Vaddr,
    end_vaddr: Vaddr,
    pub vm_flags:VmFlags,
    pages_tree:BTreeMap<Vaddr,Arc<Page>>,
    pub pagetable:Arc<PageTable>,
    pub file:Option<Arc<Inode>>,
    pub file_off:usize,
    phy_pgs_cnt:usize
}

impl ShowRdWrEx for VMA{
    fn readable(&self) -> bool {
        self.vm_flags.contains(VmFlags::VM_READ)
    }

    fn writeable(&self) -> bool {
        self.vm_flags.contains(VmFlags::VM_WRITE)
    }

    fn execable(&self) -> bool {
        self.vm_flags.contains(VmFlags::VM_EXEC)
    }
}

impl Eq for VMA {}

impl PartialEq<Self> for VMA {
    fn eq(&self, other: &Self) -> bool {
        self.start_vaddr == other.start_vaddr
    }
}

impl PartialOrd<Self> for VMA {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(if self.start_vaddr<other.start_vaddr{
            Ordering::Less
        } else if self.start_vadd==other.start_vaddr{
            Ordering::Equal
        } else {
            Ordering::Greater
        })
    }
}

impl Ord for VMA {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Debug for VMA {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        writeln!(f,"range:{:#X}=>{:#X}",self.start_vaddr.0,self.end_vaddr.0);
        writeln!(f,"flags:{:b}",self.vm_flags);
        Ok(())
    }
}

fn _vma_flags_2_pte_flags(f:VmFlags)->u8{
    ((f.bits&0b111)<<1)|PTEFlags::V.bits()
}

impl VMA {
    pub fn empty(start_addr:Vaddr,end_vaddr:Vaddr)->Self{
        Self{
            start_vaddr,
            end_vaddr,
            vm_flags: VmFlags::VM_NONE,
            pages_tree: Default::default(),
            pagetable: get_kernel_pagetable(),
            file: None,
            file_off: 0,
            phy_pgs_cnt: 0
        }
    }
    pub fn new(start_vaddr: Vaddr, end_vaddr: Vaddr,
               vm_flags:VmFlags,pagetable:Arc<PageTable>, file:Option<Arc<Inode>>,
               file_off:usize) ->Self{
        if !vm_flags.contains(VmFlags::VM_ANON){
            // file
            assert!(file.is_some());
        }
        VMA {
            start_vaddr,
            end_vaddr,
            vm_flags,
            pages_tree: Default::default(),
            pagetable,
            file,
            file_off: 0,
            phy_pgs_cnt: 0,
        }
    }
    pub fn new_anon(start_vaddr: Vaddr, end_vaddr: Vaddr,
                    vm_flags:VmFlags,pagetable:Arc<PageTable>)->Self{
        Self::new(start_vaddr,end_vaddr,vm_flags,pagetable,None,0)
    }
    pub fn new_file(start_vaddr: Vaddr, end_vaddr: Vaddr,
                    vm_flags:VmFlags,pagetable:Arc<PageTable>,file:Arc<Inode>,file_off:usize)->Self{
        Self::new(start_vaddr,end_vaddr,vm_flags,pagetable,Some(file),0)
    }
    pub fn is_anon(&self)->bool{
        self.vm_flags.contains(VmFlags::VM_ANON)
    }
    pub fn is_file(&self)->bool{
        !self.is_anon()
    }
    fn __is_dirty(&self)->bool{
        self.vm_flags.contains(VmFlags::VM_DIRTY)
    }
    fn __clear_dirty(&mut self){
        self.vm_flags.set(VmFlags::VM_DIRTY,false);
    }
    fn __set_dirty(&mut self){
        self.vm_flags.set(VmFlags::VM_DIRTY,true);
    }
    // 不会检查是否key已经存在
    pub fn _anon_insert_pages(&mut self,vaddr:Vaddr,pages:Arc<Page>){
        self.pages_tree.insert(vaddr, pages);
    }
    pub fn get_start_vaddr(&self) -> Vaddr {
        self.start_vaddr
    }
    pub fn get_end_vaddr(&self) -> Vaddr {
        self.end_vaddr
    }
    pub fn in_vma(&self, vaddr: Vaddr) ->bool{
        vaddr >=self.start_vaddr && vaddr <self.end_vaddr
    }
    pub fn get_pagetable(&self)->Arc<PageTable>{
        self.pagetable.clone()
    }
    pub fn get_flags(&self)->VmFlags{
        self.vm_flags
    }
    pub fn _vaddr_have_map(&self, vaddr:Vaddr) ->bool{
        self.pages_tree.contains_key(&vaddr)
    }
    // 为什么要返回错误值？ 可能出现映射区域超出vma范围情况
    // 输入参数要求，vaddr align && vaddr in vma
    // 可能的错误： -1 映射物理页超出vma范围
    //           -1 虚拟页存在映射
    // todo 支持force map
    pub fn _anon_map_pages(&mut self, pages:Arc<Page>, vaddr: Vaddr) ->Result<(),()>{
        debug_assert!(self.is_anon());
        debug_assert!(vaddr.is_align());
        debug_assert!(self.in_vma(vaddr));
        let pgs_cnt = order2pages(pages.get_order());
        if (self.get_end_vaddr()-vaddr.0).0 < pgs_cnt*PAGE_SIZE {
            return Err(());
        }
        // do map in pagetable
        // vma flags to pte flags
        let pte_flags = _vma_flags_2_pte_flags(self.get_flags());
        if self._vaddr_have_map(vaddr) {
            return Err(());
        }
        self.pagetable.map_pages(vaddr,pages.get_vaddr().into(),pages.get_order(),pte_flags).unwrap();
        self.phy_pgs_cnt += pgs_cnt;
        self._anon_insert_pages(vaddr,pages);
        Ok(())
    }
    // 只能按照page block的方式unmap
    pub fn _anon_unmap_pages(&mut self, vaddr:Vaddr) ->Option<Arc<Page>>{
        // find pgs from link list
        debug_assert!(self.is_anon());
        debug_assert!(vaddr.is_align());
        debug_assert!(self.in_vma(vaddr));
        match self.pages_tree.remove(&vaddr){
            None => {
                None
            }
            Some(pg) => {
                let order = pg.get_order();
                debug_assert!(self.pagetable.unmap_pages(vaddr,order).is_ok());
                Some(pg)
            }
        }
    }
    pub fn get_file_inode(&self)->Option<Arc<Inode>> {
        self.file.as_ref().map(
            |x|{
                x.clone()
            }
        )
    }
    pub fn split(&mut self,vaddr:Vaddr)->Option<Self>{
        if self.in_vma(vaddr) {
            return None;
        }
        self.end_vaddr = vaddr;
        if self.is_anon(){
            let new_anon = self.pages_tree.split_off(&vaddr);
            let mut new= Self::new_anon(
                vaddr,
                self.end_vaddr,
                self.vm_flags,
                self.get_pagetable(),
            );
            new.pages_tree = new_anon;
            Some(new)
        } else {
            let new = Self::new_file(
                vaddr,
                self.end_vaddr,
                self.vm_flags,
                self.get_pagetable(),
                self.get_file_inode().unwrap(),
                self.file_off+((vaddr-self.start_vaddr.0).0)
            );
            Some(new)
        }
    }

    pub fn _find_page(&self, vaddr:Vaddr) ->Option<Arc<Page>> {
        self.pages_tree.get(&vaddr).map(|x| {
            x.clone()
        })
    }

    // 相对map pages来说在存在映射时可以不分配物理页并且跳过，这样速度更快
    fn __fast_alloc_one_page(&mut self, vaddr:Vaddr){
        debug_assert!(self.in_vma(vaddr));
        debug_assert!(vaddr.is_align());
        if !self.pages_tree.contains_key(&vaddr) {
            let pages = alloc_one_page().unwrap();
            self.pagetable.map_one_page(vaddr, pages.get_paddr(), _vma_flags_2_pte_flags(self.get_flags()));
            self.pages_tree.insert(vaddr, pages);
        }
    }
    // 注意这个分配物理页不一定是连续的
    fn __fast_alloc_pages(&self, vaddr:Vaddr, order:usize){
        for i in vaddr.page_addr_iter(order2pages(order)*PAGE_SIZE){
            self._annon_fast_alloc_one_page(i);
        }
    }
    fn __fast_alloc_one_page_and_get(&mut self, vaddr:Vaddr) ->Arc<Page>{
        debug_assert!(self.in_vma(vaddr));
        debug_assert!(vaddr.is_align());
        if !self._vaddr_have_map(vaddr) {
            let pages = alloc_one_page().unwrap();
            self.pagetable.map_one_page(vaddr, pages.get_paddr(), _vma_flags_2_pte_flags(self.get_flags()));
            self.pages_tree.insert(vaddr, pages.clone());
            return pages;
        } else {
            self._find_page(vaddr).unwrap()
        }
    }
    // for lazy map
    pub fn _do_alloc_one_page(&mut self,vaddr:Vaddr)->Result<(),()>{
        if !vaddr.is_align() || !self.in_vma(vaddr) {
            return Err(());
        }
        if self.is_anon(){
            // alloc and map but not fill with data
            self.__fast_alloc_one_page(vaddr);
        } else {
            let f = self.file.as_ref().unwrap();
            let off = self.file_off+(vaddr-self.start_vaddr.0).0;
            let pg = self.__fast_alloc_one_page_and_get(vaddr);
            let ptr = pg.get_vaddr().get_inner() as *mut u8;
            let buf = slice_from_raw_parts_mut(ptr,PAGE_SIZE);
            let read_size = unsafe { self.read_off(&mut *buf, off) };
            assert_eq!(read_size,PAGE_SIZE);
        }
        if self.writeable(){
            // set dirty
            self.__set_dirty();
        }
        Ok(())
    }
    fn __release_one_page(&mut self,vaddr:Vaddr){
        match self.pages_tree.remove(&vaddr) {
            None => {}
            Some(pg) => {
                if self.is_anon(){
                    todo!()
                } else {
                    // file
                    let f = self.file.as_ref().unwrap();
                    let off = self.file_off+(vaddr-self.start_vaddr.0).0;
                    let ptr = pg.get_vaddr().get_inner() as *const u8;
                    let buf = slice_from_raw_parts(ptr,PAGE_SIZE);
                    let write_size = unsafe { self.write_off(&*buf, off) };
                    assert_eq!(write_size, PAGE_SIZE);
                }
            }
        }
    }
    pub fn _release_all_page(&mut self){
        todo!()
    }
}

// // todo 安全性 是否需要加锁才能访问page
// impl ReadWriteOffUnsafe<u8> for VMA {
//     unsafe fn read_off(&self, buf: &mut [u8], off: usize) -> usize {
//         let size = 1;
//         let buf_size = buf.len() * size;
//         assert!(Vaddr(off).is_align_n(size));
//         assert!(self.start_vaddr+buf_size+off < self.end_vaddr);
//         assert!(self.start_vaddr+off < self.end_vaddr);
//         let start = self.start_vaddr;
//         let mut page_now = self.__fast_alloc_one_page_and_get(start);
//         page_now.seek(SeekFrom::Start(off as u64));
//         let mut buf_index:usize = 0;
//         while buf_index < buf.len() {
//             let read_len = page_now.read(&mut buf[buf_index..]).unwrap();
//             if read_len == 0 {
//                 // change pages
//                 let vaddr_now = start+off + buf_index*size;
//                 if buf_index!=buf.len(){
//                     assert!(vaddr_now.is_align());
//                 }
//                 page_now = self.__fast_alloc_one_page_and_get(vaddr_now);
//                 page_now.seek(SeekFrom::Start(0));
//             } else {
//                 buf_index+=read_len;
//             }
//         }
//         buf_size
//     }
//
//     unsafe fn write_off(&self, buf: &[u8], off: usize) -> usize {
//         let size = 1;
//         let buf_size = buf.len() * size;
//         assert!(Vaddr(off).is_align_n(size));
//         assert!(self.start_vaddr+buf_size+off < self.end_vaddr);
//         assert!(self.start_vaddr+off < self.end_vaddr);
//         let start = self.start_vaddr;
//         let mut page_now = self.__fast_alloc_one_page_and_get(start);
//         page_now.seek(SeekFrom::Start(off as u64));
//         let mut buf_index:usize = 0;
//         while buf_index < buf.len() {
//             let write_len = page_now.write(&buf[buf_index..]).unwrap();
//             if write_len == 0 {
//                 // change pages
//                 let vaddr_now = start+off + buf_index*size;
//                 if buf_index!=buf.len(){
//                     assert!(vaddr_now.is_align());
//                 }
//                 page_now = self.__fast_alloc_one_page_and_get(vaddr_now);
//                 page_now.seek(SeekFrom::Start(0));
//             } else {
//                 buf_index+= write_len;
//             }
//         }
//         buf_size
//     }
// }