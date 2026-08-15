use block::{Block, BlockParent, BlockReferenceList};
use block_client::{
    block_ref::BlockRef,
    blocks::settings::{ActivationCondition, Settings, SettingsOperation},
    BlockClient, BlockHandle, ReferenceList,
};
use uuid::Uuid;

/// The workspace's single settings block, sitting at the root of the block
/// tree. Every block type's settings are registered inside it instead of each
/// type having a root block of its own, so the root listing stays down to one
/// entry no matter how many kinds of settings accumulate.
struct RootSettings {
    roots: ReferenceList,
    block: Option<BlockHandle<Settings>>,
}

impl RootSettings {
    fn new(client: &BlockClient) -> Self {
        Self {
            roots: client.watch_references(BlockReferenceList::Roots),
            block: None,
        }
    }

    /// Looks the settings block up in the root listing, which is only there
    /// to be searched once the workspace has answered.
    fn find(&mut self, client: &BlockClient) -> Option<&BlockHandle<Settings>> {
        if self.block.is_none() {
            let found = self
                .roots
                .read()
                .into_iter()
                .find(|reference| reference.block_type == Settings::TYPE_ID)?;
            self.block = Some(client.get_block::<Settings>(found.id));
        }
        self.block.as_ref()
    }

    /// The settings block to write to, creating it at the root when the tree
    /// has none. Nothing is created before the root listing has loaded, so a
    /// workspace that already has one is never given a second.
    fn ensure(&mut self, client: &BlockClient) -> Option<&BlockHandle<Settings>> {
        if self.find(client).is_none() && self.roots.is_loaded() {
            let settings = client.create_block(Settings::new());
            settings.set_parent(BlockParent::Root);
            self.block = Some(settings);
        }
        self.block.as_ref()
    }
}

/// Finds, or lazily creates, the settings block of type `T` for this
/// workspace. `T`'s data sits under the shared root [`Settings`] block rather
/// than being a root block of its own, indexed by `T::TYPE_ID` and by which
/// client is asking, so different devices could someday keep different
/// settings for the same block type.
pub(super) struct RootSetting<T: Block> {
    settings: RootSettings,
    block: Option<BlockHandle<T>>,
}

impl<T: Block + Default> RootSetting<T> {
    pub(super) fn new(client: &BlockClient) -> Self {
        Self {
            settings: RootSettings::new(client),
            block: None,
        }
    }

    /// The block found so far, if any.
    pub(super) fn block(&self) -> Option<&BlockHandle<T>> {
        self.block.as_ref()
    }

    /// Looks `T`'s settings up in the root `Settings` block, preferring an
    /// entry registered for `client_id` over the fallback one.
    pub(super) fn find(
        &mut self,
        client: &BlockClient,
        client_id: Uuid,
    ) -> Option<&BlockHandle<T>> {
        if self.block.is_none() {
            let id = {
                let settings = self.settings.find(client)?.read()?;
                settings.resolve(T::TYPE_ID, client_id)?.as_direct()?
            };
            self.block = Some(client.get_block::<T>(id));
        }
        self.block.as_ref()
    }

    /// The block to write to, creating it - and registering it in the root
    /// `Settings` block under the `Fallback` activation - the first time this
    /// type is edited in this workspace.
    pub(super) fn ensure(
        &mut self,
        client: &BlockClient,
        client_id: Uuid,
    ) -> Option<&BlockHandle<T>> {
        if self.find(client, client_id).is_some() {
            return self.block.as_ref();
        }
        let settings = self.settings.ensure(client)?;
        let block = client.create_block(T::default());
        settings.operate(SettingsOperation::SetEntry {
            block_type: T::TYPE_ID,
            activation: ActivationCondition::Fallback,
            block: BlockRef::Direct(block.id()),
        });
        block.set_parent(BlockParent::Uuid(settings.id()));
        self.block = Some(block);
        self.block.as_ref()
    }
}
