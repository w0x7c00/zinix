use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use core::fmt::{Debug, Formatter, Write};
use crate::consts::PAGE_SIZE;

pub const  AT_NULL      : usize = 0 ;    /* end of vector */
pub const  AT_IGNORE    : usize = 1 ;    /* entry should be ignored */
pub const  AT_EXECFD    : usize = 2 ;    /* file descriptor of program */
pub const  AT_PHDR      : usize = 3 ;    /* program headers for program */
pub const  AT_PHENT     : usize = 4 ;    /* size of program header entry */
pub const  AT_PHNUM     : usize = 5 ;    /* number of program headers */
pub const  AT_PAGESZ    : usize = 6 ;    /* system page size */
pub const  AT_BASE      : usize = 7 ;    /* base address of interpreter */
pub const  AT_FLAGS     : usize = 8 ;    /* flags */
pub const  AT_ENTRY     : usize = 9 ;    /* entry point of program */
pub const  AT_NOTELF    : usize = 10;    /* program is not ELF */
pub const  AT_UID       : usize = 11;    /* real uid */
pub const  AT_EUID      : usize = 12;    /* effective uid */
pub const  AT_GID       : usize = 13;    /* real gid */
pub const  AT_EGID      : usize = 14;    /* effective gid */
pub const  AT_PLATFORM  : usize = 15;  /* string identifying CPU for optimizations */
pub const  AT_HWCAP     : usize = 16;    /* arch dependent hints at CPU capabilities */
pub const  AT_CLKTCK    : usize = 17;    /* frequency at which times() increments */
/* AT_* values 18 through 22 are reserved */
pub const AT_SECURE     : usize = 23;   /* secure mode boolean */
pub const AT_BASE_PLATFORM : usize = 24;     /* string identifying real platform, may
                                 * differ from AT_PLATFORM. */
pub const AT_RANDOM     : usize = 25;    /* address of 16 random bytes */
pub const AT_HWCAP2     : usize = 26;    /* extension of AT_HWCAP */

pub const AT_EXECFN     : usize = 31;   /* filename of program */
/* Pointer to the global system page used for system calls and other
   nice things.  */
pub const AT_SYSINFO	: usize = 32;
pub const AT_SYSINFO_EHDR: usize = 	33;

pub struct AuxHeader{
    pub aux_type:usize,
    pub value:usize
}

impl Debug for AuxHeader {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("AuxHeader: type:{} value:0x{:X}", self.aux_type, self.value))
    }
}

pub fn make_auxv(entry_size:usize, ph_cnt: usize, entry:usize)->Vec<AuxHeader>{
    let mut auxv:Vec<AuxHeader> = vec![];
    auxv.push(AuxHeader{aux_type: AT_PHENT, value: entry_size});
    auxv.push(AuxHeader{aux_type: AT_PHNUM, value: ph_cnt});
    auxv.push(AuxHeader{aux_type: AT_PAGESZ, value: PAGE_SIZE});
    auxv.push(AuxHeader{aux_type: AT_BASE, value: 0 });
    auxv.push(AuxHeader{aux_type: AT_FLAGS, value: 0 });
    auxv.push(AuxHeader{aux_type: AT_ENTRY, value: entry});
    auxv.push(AuxHeader{aux_type: AT_UID, value: 0});
    auxv.push(AuxHeader{aux_type: AT_EUID, value: 0});
    auxv.push(AuxHeader{aux_type: AT_GID, value: 0});
    auxv.push(AuxHeader{aux_type: AT_EGID, value: 0});
    auxv.push(AuxHeader{aux_type: AT_PLATFORM, value: 0});
    auxv.push(AuxHeader{aux_type: AT_HWCAP, value: 0});
    auxv.push(AuxHeader{aux_type: AT_CLKTCK, value: 100});
    auxv.push(AuxHeader{aux_type: AT_SECURE, value: 0});
    auxv.push(AuxHeader{aux_type: AT_NOTELF, value: 0x112d});
    auxv
}