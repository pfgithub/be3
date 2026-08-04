use block::{Block, NoHistory};
use logicgame::{
    execution::UnlinkedComponent,
    grid::{ComponentKind, ComponentPort, ComponentSubgraph, GeometryError, Size},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One logic grid compiled into a program, together with the shape it takes
/// when it is placed inside another grid. Calls to other components name the
/// compiled block that holds them, so a whole circuit can be linked by
/// following references.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct CompiledLogic {
    /// The logic grid block this was compiled from.
    source: Uuid,
    /// The outline the subcomponent occupies, in grid cells.
    size: Size,
    ports: Vec<ComponentPort>,
    program: UnlinkedComponent,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum CompiledLogicOperation {
    Replace { compiled: CompiledLogic },
}

impl CompiledLogic {
    pub fn new(
        source: Uuid,
        size: Size,
        ports: Vec<ComponentPort>,
        program: UnlinkedComponent,
    ) -> Self {
        Self {
            source,
            size,
            ports,
            program,
        }
    }

    pub fn source(&self) -> Uuid {
        self.source
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn ports(&self) -> &[ComponentPort] {
        &self.ports
    }

    pub fn program(&self) -> &UnlinkedComponent {
        &self.program
    }

    /// The compiled blocks this program calls. Every one of them has to be
    /// loaded before the program can run.
    pub fn calls(&self) -> &[Uuid] {
        &self.program.components
    }

    /// The independent input-to-output paths through the component. A caller
    /// only runs the ones whose outputs it is actually wired to.
    pub fn subgraphs(&self) -> Vec<ComponentSubgraph> {
        self.program
            .subgraphs
            .iter()
            .map(|subgraph| ComponentSubgraph {
                inputs: subgraph.inputs.clone(),
                outputs: subgraph.outputs.clone(),
            })
            .collect()
    }

    /// The component to place in a grid to call this program. `id` is this
    /// block's own ID and `name` the label drawn in the middle of it.
    pub fn placement(&self, id: Uuid, name: &str) -> Result<ComponentKind, GeometryError> {
        let mut kind = ComponentKind::subcomponent_with_subgraphs(
            id,
            self.size,
            self.ports.clone(),
            self.subgraphs(),
        )?;
        if let ComponentKind::Subcomponent { name: label, .. } = &mut kind {
            label.clear();
            label.push_str(name);
        }
        Ok(kind)
    }
}

impl Block for CompiledLogic {
    type Operation = CompiledLogicOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x636f_6d70_696c_6564_2d6c_6f67_6963_0101);

    fn apply_operation(compiled: &mut Self, operation: &Self::Operation) {
        match operation {
            CompiledLogicOperation::Replace {
                compiled: replacement,
            } => *compiled = replacement.clone(),
        }
    }

    fn implicit_name(&self) -> String {
        "Compiled Logic".into()
    }

    fn references(&self) -> Vec<Uuid> {
        self.calls().to_vec()
    }
}

#[cfg(test)]
mod tests;
