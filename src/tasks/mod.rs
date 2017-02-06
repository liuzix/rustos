pub mod threads;
pub mod scheduler;

use self::scheduler::Scheduler;

lazy_static! {
    pub static ref SCHEDULER: Scheduler = Scheduler::new();
}