use crate::sparse_set::SparseSet;
use crate::storage::{Entities, EntityId, StorageId};
use alloc::vec::Vec;
use core::any::Any;

pub(super) trait UnknownStorage {
    fn delete(&mut self, entity: EntityId, storage_to_unpack: &mut Vec<StorageId>);
    fn clear(&mut self);
    fn unpack(&mut self, entity: EntityId);
    fn any(&self) -> &dyn Any;
    fn any_mut(&mut self) -> &mut dyn Any;
}

impl dyn UnknownStorage {
    pub(crate) fn sparse_set<T: 'static>(&self) -> Option<&SparseSet<T>> {
        self.any().downcast_ref()
    }
    pub(crate) fn sparse_set_mut<T: 'static>(&mut self) -> Option<&mut SparseSet<T>> {
        self.any_mut().downcast_mut()
    }
    pub(crate) fn entities(&self) -> Option<&Entities> {
        self.any().downcast_ref()
    }
    pub(crate) fn entities_mut(&mut self) -> Option<&mut Entities> {
        self.any_mut().downcast_mut()
    }
    pub(crate) fn unique<T: 'static>(&self) -> Option<&T> {
        self.any().downcast_ref()
    }
    pub(crate) fn unique_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.any_mut().downcast_mut()
    }
}
