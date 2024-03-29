use alloc::string::String;
use core::ptr::{addr_of, addr_of_mut};
use fatfs::{Date, DateTime};
use log::error;
use xmas_elf::header::Data;
use crate::consts::{DIRECT_MAP_START, MAX_ORDER, PAGE_OFFSET, PAGE_SIZE};
use crate::mm::addr::Vaddr;
use crate::pre::InnerAccess;

pub fn addr_page_align_upper(addr:usize) ->usize{
    let mut ret = addr-(addr&PAGE_SIZE);
    if (ret&PAGE_SIZE) != 0 {
        ret+=PAGE_SIZE;
    }
    return ret;
}

pub fn addr_page_align_lower(addr:usize)->usize{
    return addr-(addr&PAGE_SIZE);
}

pub fn vaddr2paddr(addr:usize)->usize{
    return addr-DIRECT_MAP_START;
}

pub fn paddr2vaddr(addr:usize)->usize{
    return addr+DIRECT_MAP_START;
}

pub fn addr_get_ppn0(vaddr:usize)->usize{
    (vaddr>>12)&0x1FF
}

pub fn addr_get_ppn1(vaddr:usize)->usize{
    (vaddr>>21)&0x1FF
}

pub fn addr_get_ppn2(vaddr:usize)->usize{
    (vaddr>>30)&0x1FF
}

pub unsafe  fn get_usize_by_addr(vaddr:usize)->usize{
    let ptr = vaddr as *mut usize;
    ptr.read_volatile()
}

pub unsafe fn set_usize_by_addr(vaddr:usize,val:usize){
    let ptr = vaddr as *mut usize;
    ptr.write_volatile(val);
}

pub unsafe fn memcpy(dest:usize,src: usize,len:usize){
    for i in 0..len{
        *((dest+i) as *mut u8) = *((src+i) as *mut u8);
    }
}

pub fn order2pages(order:usize) ->usize{
    return if order < MAX_ORDER {
        1 << order
    } else {
        0
    }
}

pub fn pages2order(pages:usize) ->usize{
    for i in 0..MAX_ORDER{
        if order2pages(i)>=pages{
            return i;
        }
    }
    error!("pg2order fail");
    return MAX_ORDER;
}

pub fn convert_cstr_from_ptr(ptr: *const u8) -> String{
    let mut s = String::new();
    let mut ptr_probe = ptr as usize;
    loop {
        let c = unsafe{*(ptr_probe as *const u8)};
        if c ==0 {
            break;
        }
        s.push(c as char);
        ptr_probe +=1;
    }
    s
}

pub fn convert_cstr_from_vaddr(vaddr:Vaddr)->String{
    convert_cstr_from_ptr(vaddr.get_inner() as *const u8)
}

const D_SECOND:u64 = 3600*24;
const M_SECOND:u64 = D_SECOND*30;
const Y_SECOND:u64 = D_SECOND*365;

pub fn date2second(d:Date)->u64{
    let y = d.year - 1970;
    let m = d.month - 1;
    let d = d.day - 1;
    y as u64*Y_SECOND + m as u64 *M_SECOND + d  as u64 *D_SECOND
}

pub fn datetime2second(d:DateTime)->u64{
    let datesecond = date2second(d.date);
    let timesecond = d.time.hour as u64 * 3600 + d.time.min as u64 * 60 + d.time.sec as u64;
    datesecond + timesecond
}