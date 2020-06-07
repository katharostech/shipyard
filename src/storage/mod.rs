mod all;
mod entity;
mod unique;

pub use all::{AllStorages, DeleteAny};
pub use entity::{Entities, EntitiesIter, EntityId};

pub(crate) use all::TypeIdHasher;

use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::error;
use crate::sparse_set::SparseSet;
use crate::unknown_storage::UnknownStorage;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::TypeId;
use core::cmp::Ordering;
use unique::Unique;

/// Currently unused it'll replace `TypeId` in `AllStorages` in a future version.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum StorageId {
    TypeId(TypeId),
    Custom(u64),
}

// TODO: Currently custom elements sort as less than TypeId
impl Ord for StorageId {
    fn cmp(&self, other: &Self) -> Ordering {
        match self {
            StorageId::TypeId(id) => match other {
                StorageId::TypeId(other_id) => id.cmp(other_id),
                StorageId::Custom(_) => Ordering::Greater,
            },
            StorageId::Custom(id) => match other {
                StorageId::Custom(other_id) => id.cmp(other_id),
                StorageId::TypeId(_) => Ordering::Less,
            },
        }
    }
}

impl PartialOrd for StorageId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<TypeId> for StorageId {
    fn from(type_id: TypeId) -> Self {
        StorageId::TypeId(type_id)
    }
}

impl From<u64> for StorageId {
    fn from(int: u64) -> Self {
        StorageId::Custom(int)
    }
}

/// Abstract away `T` from `AtomicRefCell<T>` to be able to store
/// different types in a `HashMap<TypeId, Storage>`.  
/// and box the `AtomicRefCell` so it doesn't move when the `HashMap` reallocates
pub(crate) struct Storage(pub(super) Box<AtomicRefCell<dyn UnknownStorage>>);

#[cfg(not(feature = "non_send"))]
unsafe impl Send for Storage {}

unsafe impl Sync for Storage {}

impl Storage {
    /// Creates a new `Storage` storing elements of type T.
    pub(crate) fn new<T: 'static + Send + Sync>() -> Self {
        let sparse_set = SparseSet::<T>::new();
        #[cfg(feature = "std")]
        {
            Storage(Box::new(AtomicRefCell::new(sparse_set, None, true)))
        }
        #[cfg(not(feature = "std"))]
        {
            Storage(Box::new(AtomicRefCell::new(sparse_set)))
        }
    }
    #[cfg(feature = "non_send")]
    pub(crate) fn new_non_send<T: 'static + Sync>(world_thread_id: std::thread::ThreadId) -> Self {
        let sparse_set = SparseSet::<T>::new();
        Storage(Box::new(AtomicRefCell::new(
            sparse_set,
            Some(world_thread_id),
            true,
        )))
    }
    #[cfg(feature = "non_sync")]
    pub(crate) fn new_non_sync<T: 'static + Send>() -> Self {
        let sparse_set = SparseSet::<T>::new();
        Storage(Box::new(AtomicRefCell::new(sparse_set, None, false)))
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    pub(crate) fn new_non_send_sync<T: 'static>(world_thread_id: std::thread::ThreadId) -> Self {
        let sparse_set = SparseSet::<T>::new();
        Storage(Box::new(AtomicRefCell::new(
            sparse_set,
            Some(world_thread_id),
            false,
        )))
    }
    pub(crate) fn new_unique<T: 'static + Send + Sync>(component: T) -> Self {
        #[cfg(feature = "std")]
        {
            Storage(Box::new(AtomicRefCell::new(Unique(component), None, true)))
        }
        #[cfg(not(feature = "std"))]
        {
            Storage(Box::new(AtomicRefCell::new(Unique(component))))
        }
    }
    #[cfg(feature = "non_send")]
    pub(crate) fn new_unique_non_send<T: 'static + Sync>(
        component: T,
        world_thread_id: std::thread::ThreadId,
    ) -> Self {
        Storage(Box::new(AtomicRefCell::new(
            Unique(component),
            Some(world_thread_id),
            true,
        )))
    }
    #[cfg(feature = "non_sync")]
    pub(crate) fn new_unique_non_sync<T: 'static + Send>(component: T) -> Self {
        Storage(Box::new(AtomicRefCell::new(Unique(component), None, false)))
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    pub(crate) fn new_unique_non_send_sync<T: 'static>(
        component: T,
        world_thread_id: std::thread::ThreadId,
    ) -> Self {
        Storage(Box::new(AtomicRefCell::new(
            Unique(component),
            Some(world_thread_id),
            false,
        )))
    }
    /// Immutably borrows the component container.
    pub(crate) fn sparse_set<T: 'static>(
        &self,
    ) -> Result<Ref<'_, SparseSet<T>>, error::GetStorage> {
        Ref::try_map(
            self.0.try_borrow().map_err(|borrow| {
                error::GetStorage::StorageBorrow((core::any::type_name::<T>(), borrow))
            })?,
            |unknown| {
                if let Some(storage) = unknown.sparse_set::<T>() {
                    Ok(storage)
                } else {
                    Err(error::GetStorage::Unique {
                        name: core::any::type_name::<T>(),
                        borrow: error::Borrow::Shared,
                    })
                }
            },
        )
    }
    /// Mutably borrows the component container.
    pub(crate) fn sparse_set_mut<T: 'static>(
        &self,
    ) -> Result<RefMut<'_, SparseSet<T>>, error::GetStorage> {
        RefMut::try_map(
            self.0.try_borrow_mut().map_err(|borrow| {
                error::GetStorage::StorageBorrow((core::any::type_name::<T>(), borrow))
            })?,
            |unknown| {
                if let Some(storage) = unknown.sparse_set_mut::<T>() {
                    Ok(storage)
                } else {
                    Err(error::GetStorage::Unique {
                        name: core::any::type_name::<T>(),
                        borrow: error::Borrow::Unique,
                    })
                }
            },
        )
    }
    /// Immutably borrows entities' storage.
    pub(crate) fn entities(&self) -> Result<Ref<'_, Entities>, error::Borrow> {
        Ok(Ref::map(self.0.try_borrow()?, |unknown| {
            unknown.entities().unwrap()
        }))
    }
    /// Mutably borrows entities' storage.
    pub(crate) fn entities_mut(&self) -> Result<RefMut<'_, Entities>, error::Borrow> {
        Ok(RefMut::map(self.0.try_borrow_mut()?, |unknown| {
            unknown.entities_mut().unwrap()
        }))
    }
    pub(crate) fn unique<T: 'static>(&self) -> Result<Ref<'_, T>, error::GetStorage> {
        Ref::try_map(
            self.0.try_borrow().map_err(|borrow| {
                error::GetStorage::StorageBorrow((core::any::type_name::<T>(), borrow))
            })?,
            |unknown| {
                if let Some(storage) = unknown.unique::<T>() {
                    Ok(storage)
                } else {
                    Err(error::GetStorage::NonUnique((
                        core::any::type_name::<T>(),
                        error::Borrow::Shared,
                    )))
                }
            },
        )
    }
    pub(crate) fn unique_mut<T: 'static>(&self) -> Result<RefMut<'_, T>, error::GetStorage> {
        RefMut::try_map(
            self.0.try_borrow_mut().map_err(|borrow| {
                error::GetStorage::StorageBorrow((core::any::type_name::<T>(), borrow))
            })?,
            |unknown| {
                if let Some(storage) = unknown.unique_mut::<T>() {
                    Ok(storage)
                } else {
                    Err(error::GetStorage::NonUnique((
                        core::any::type_name::<T>(),
                        error::Borrow::Unique,
                    )))
                }
            },
        )
    }
    /// Mutably borrows the container and delete `index`.
    pub(crate) fn delete(
        &mut self,
        entity: EntityId,
        storage_to_unpack: &mut Vec<StorageId>,
    ) -> Result<(), error::Borrow> {
        self.0.try_borrow_mut()?.delete(entity, storage_to_unpack);
        Ok(())
    }
    pub(crate) fn unpack(&mut self, entity: EntityId) -> Result<(), error::Borrow> {
        self.0.try_borrow_mut()?.unpack(entity);
        Ok(())
    }
    pub(crate) fn clear(&mut self) -> Result<(), error::Borrow> {
        self.0.try_borrow_mut()?.clear();
        Ok(())
    }
}

#[test]
fn delete() {
    let mut storage = Storage::new::<&'static str>();
    let mut entity_id = EntityId::zero();
    let mut storage_to_unpack = Vec::new();
    entity_id.set_index(5);
    storage.sparse_set_mut().unwrap().insert("test5", entity_id);
    entity_id.set_index(10);
    storage
        .sparse_set_mut()
        .unwrap()
        .insert("test10", entity_id);
    entity_id.set_index(1);
    storage.sparse_set_mut().unwrap().insert("test1", entity_id);
    entity_id.set_index(5);
    storage.delete(entity_id, &mut storage_to_unpack).unwrap();
    assert_eq!(storage.sparse_set::<&str>().unwrap().get(entity_id), None);
    entity_id.set_index(10);
    assert_eq!(
        storage.sparse_set::<&str>().unwrap().get(entity_id),
        Some(&"test10")
    );
    entity_id.set_index(1);
    assert_eq!(
        storage.sparse_set::<&str>().unwrap().get(entity_id),
        Some(&"test1")
    );
    entity_id.set_index(10);
    storage.delete(entity_id, &mut storage_to_unpack).unwrap();
    entity_id.set_index(1);
    storage.delete(entity_id, &mut storage_to_unpack).unwrap();
    entity_id.set_index(5);
    assert_eq!(storage.sparse_set::<&str>().unwrap().get(entity_id), None);
    entity_id.set_index(10);
    assert_eq!(storage.sparse_set::<&str>().unwrap().get(entity_id), None);
    entity_id.set_index(1);
    assert_eq!(storage.sparse_set::<&str>().unwrap().get(entity_id), None);
}
