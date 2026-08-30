use block::{Block, BlockParent, BlockReferenceList};
use block_client::{
    block_ref::BlockRef,
    blocks::settings::{ActivationCondition, Settings, SettingsOperation},
    BlockClient, BlockHandle, ReferenceList,
};
use uuid::Uuid;

pub(crate) struct RootSettings {
    roots: ReferenceList,
    block: Option<BlockHandle<Settings>>,
}

impl RootSettings {
    pub(crate) fn new(client: &BlockClient) -> Self {
        Self {
            roots: client.watch_references(BlockReferenceList::Roots),
            block: None,
        }
    }

    pub(crate) fn find(&mut self, client: &BlockClient) -> Option<&BlockHandle<Settings>> {
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

    pub(crate) fn ensure(&mut self, client: &BlockClient) -> Option<&BlockHandle<Settings>> {
        if self.find(client).is_none() && self.roots.is_loaded() {
            let settings = client.create_block(Settings::new());
            settings.set_parent(BlockParent::Root);
            self.block = Some(settings);
        }
        self.block.as_ref()
    }
}

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

    pub(super) fn block(&self) -> Option<&BlockHandle<T>> {
        self.block.as_ref()
    }

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
