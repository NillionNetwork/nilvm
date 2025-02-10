use crate::stateful::preprocessing_scheduler::SchedulerHandle;
use node_api::preprocessing::rust::PreprocessingElement;

#[cfg_attr(test, mockall::automock)]
pub(crate) trait PreprocessingSchedulingService: Send + Sync + 'static {
    fn notify_used_elements(&self, elements: &[PreprocessingElement]);
}

pub(crate) struct DefaultPreprocessingSchedulingService {
    handle: SchedulerHandle,
}

impl DefaultPreprocessingSchedulingService {
    pub(crate) fn new(handle: SchedulerHandle) -> Self {
        Self { handle }
    }
}

impl PreprocessingSchedulingService for DefaultPreprocessingSchedulingService {
    fn notify_used_elements(&self, elements: &[PreprocessingElement]) {
        self.handle.notify_used_elements(elements);
    }
}
