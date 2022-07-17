// VM distribute
// 虚拟内存空间分配需要保证在sv39及其以上都支持，最低要求sv39，所以地址分区在只看39位时不能出现交叉情况
// User Space ：256GB
pub static USER_SPACE_START:usize = 0x0;
pub static USER_SPACE_END:usize = 0x4000000000;

// vmemmap : 4GB
pub static VMEMMAP_START:usize = 0xffffffc700000000;
pub static VMEMMAP_END:usize = 0xffffffc800000000;

// direct map : 124GB
pub static DIRECT_MAP_START:usize = 0xffffffd800000000;
pub static DIRECT_MAP_END:usize = 0xfffffff700000000;

pub const PAGE_SIZE:usize = 4096;
pub const PAGE_OFFSET:usize = 12;

pub const CPUS:usize = 2;

// PF allocator
pub const MAX_ORDER:usize = 11;
pub const MAX_ORDER_NR_PAGES:usize = 1<<(MAX_ORDER-1);
