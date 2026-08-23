use std::{collections::BTreeMap, fmt};

use block::{Block, NoHistory};
use logicgame::{
    execution::{GenerationError, UnlinkedComponent},
    grid::{
        ComponentKind, ComponentPort, ComponentSide, ComponentSubgraph, ConnectionDirection,
        GeometryError, LogicGrid as Grid, Point, Size,
    },
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

                                                    
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileError {
                                                                              
    Empty,
                                                                       
    TooLarge,
                                                                   
    PortWithoutLead,
    Geometry(GeometryError),
    Generation(GenerationError),
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("An empty grid cannot be compiled"),
            Self::TooLarge => formatter.write_str("The circuit is too large to compile"),
            Self::PortWithoutLead => {
                formatter.write_str("An input or output has no lead out of the circuit")
            }
            Self::Geometry(error) => write!(formatter, "Invalid component shape: {error:?}"),
            Self::Generation(error) => write!(formatter, "Cannot compile circuit: {error:?}"),
        }
    }
}

impl From<GeometryError> for CompileError {
    fn from(error: GeometryError) -> Self {
        Self::Geometry(error)
    }
}

impl From<GenerationError> for CompileError {
    fn from(error: GenerationError) -> Self {
        Self::Generation(error)
    }
}

                                                                            
                                                                             
                                                                       
                         
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct CompiledLogic {
                                                    
    source: Uuid,
                                                             
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

                                                                            
                                                                              
                
    pub fn compile(source: Uuid, grid: &Grid) -> Result<Self, CompileError> {
        let bounds = grid.bounds().ok_or(CompileError::Empty)?;
        let width = i64::try_from(bounds.width()).map_err(|_| CompileError::TooLarge)?;
        let height = i64::try_from(bounds.height()).map_err(|_| CompileError::TooLarge)?;
        let ports = boundary_ports(grid, bounds.min)?;
        let program = UnlinkedComponent::from_graph(grid, &grid.generate_graph())?;
        Ok(Self::new(source, Size::new(width, height), ports, program))
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

                                                                           
                                          
    pub fn calls(&self) -> &[Uuid] {
        &self.program.components
    }

                                                                             
                                                                 
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

    fn references(&self) -> Vec<Uuid> {
        self.calls().to_vec()
    }
}

                                                                           
                                                                             
                                                          
fn boundary_ports(grid: &Grid, min: Point) -> Result<Vec<ComponentPort>, CompileError> {
    let input_indices = dense_indices(grid.components().filter_map(
        |component| match component.kind {
            ComponentKind::Input { id, .. } => Some(id),
            _ => None,
        },
    ));
    let output_indices = dense_indices(grid.components().filter_map(
        |component| match component.kind {
            ComponentKind::Output { id, .. } => Some(id),
            _ => None,
        },
    ));

    let mut ports = Vec::new();
    for component in grid.components() {
        let (direction, index, scale, label) = match &component.kind {
            ComponentKind::Input { id, scale, label } => (
                ConnectionDirection::Input,
                input_indices[id],
                *scale,
                label.clone(),
            ),
            ComponentKind::Output { id, scale, label } => (
                ConnectionDirection::Output,
                output_indices[id],
                *scale,
                label.clone(),
            ),
            _ => continue,
        };
        let lead = component.lead().ok_or(CompileError::PortWithoutLead)?;
        let offset = match lead.side {
            ComponentSide::Top | ComponentSide::Bottom => min.x,
            ComponentSide::Right | ComponentSide::Left => min.y,
        };
        ports.push(ComponentPort {
            direction,
            index,
            scale,
            side: lead.side,
            start: lead.start - offset,
            end: lead.end - offset,
            label,
        });
    }
    ports.sort_by_key(|port| (port.direction, port.index));
    Ok(ports)
}

                                                                              
                                                 
fn dense_indices<T: Ord>(ids: impl IntoIterator<Item = T>) -> BTreeMap<T, usize> {
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids.into_iter()
        .enumerate()
        .map(|(index, id)| (id, index))
        .collect()
}

#[cfg(test)]
mod tests;
