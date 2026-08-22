use uuid::Uuid;
use wasmi::{Config, Engine, Instance, Linker, Memory, Module, Store, TypedFunc};

pub use game_api::{GameAction, GameActionOption, GameRequest, GameScreen};

/// How much a single `show` call may execute before the interpreter stops
/// it. A game replays its whole log on every call, so this is generous - it
/// exists so a module that never returns cannot take the app's frame with
/// it, not to budget honest games.
const FUEL: u64 = 100_000_000;

/// One game, as the WebAssembly module that plays it. The module is the
/// whole game: the app knows nothing about turns, legality or win
/// conditions, and asks the module what a player sees by replaying the
/// action log into it. Every call runs in an instance of its own, so a game
/// keeps no state between calls beyond the log it is given.
pub struct Game {
    engine: Engine,
    module: Module,
    name: String,
}

impl Game {
    pub fn load(module: &[u8]) -> Result<Self, String> {
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, module)
            .map_err(|error| format!("this is not a game module: {error}"))?;
        let mut game = Self {
            engine,
            module,
            name: String::new(),
        };
        game.name = game.ask_name()?;
        Ok(game)
    }

    /// What the module calls itself, which is the only name the app has for
    /// a game it discovered rather than shipped.
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn show(&self, actions: &[GameAction], player: Uuid) -> Result<GameScreen, String> {
        let request = bincode::serialize(&GameRequest {
            actions: actions.to_vec(),
            player,
        })
        .map_err(|error| format!("the action log could not be encoded: {error}"))?;
        let length = u32::try_from(request.len())
            .map_err(|_| "the action log is too long for a game module".to_owned())?;

        let (mut store, instance, memory) = self.instantiate()?;
        let allocate: TypedFunc<u32, u32> = export(&store, &instance, "allocate")?;
        let show: TypedFunc<(u32, u32), u64> = export(&store, &instance, "show")?;

        let pointer = allocate
            .call(&mut store, length)
            .map_err(|error| format!("this game could not take the action log: {error}"))?;
        memory
            .write(&mut store, pointer as usize, &request)
            .map_err(|error| format!("this game could not take the action log: {error}"))?;
        let answer = show
            .call(&mut store, (pointer, length))
            .map_err(|error| format!("this game stopped: {error}"))?;

        let screen = read(&store, &memory, answer)?;
        bincode::deserialize(&screen)
            .map_err(|error| format!("this game answered with nothing readable: {error}"))
    }

    fn ask_name(&self) -> Result<String, String> {
        let (mut store, instance, memory) = self.instantiate()?;
        let name: TypedFunc<(), u64> = export(&store, &instance, "name")?;
        let answer = name
            .call(&mut store, ())
            .map_err(|error| format!("this game would not name itself: {error}"))?;
        String::from_utf8(read(&store, &memory, answer)?)
            .map_err(|_| "this game's name is not text".to_owned())
    }

    fn instantiate(&self) -> Result<(Store<()>, Instance, Memory), String> {
        let mut store = Store::new(&self.engine, ());
        store
            .set_fuel(FUEL)
            .map_err(|error| format!("this game could not be given fuel: {error}"))?;
        let instance = Linker::<()>::new(&self.engine)
            .instantiate_and_start(&mut store, &self.module)
            .map_err(|error| format!("this game would not start: {error}"))?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| "this game module exports no memory".to_owned())?;
        Ok((store, instance, memory))
    }
}

fn export<Parameters: wasmi::WasmParams, Results: wasmi::WasmResults>(
    store: &Store<()>,
    instance: &Instance,
    name: &str,
) -> Result<TypedFunc<Parameters, Results>, String> {
    instance
        .get_typed_func(store, name)
        .map_err(|error| format!("this game module has no usable {name}: {error}"))
}

/// Reads back what a module handed over: a pointer and a length packed into
/// the one integer a call can answer with.
fn read(store: &Store<()>, memory: &Memory, answer: u64) -> Result<Vec<u8>, String> {
    let pointer = (answer >> 32) as usize;
    let length = (answer & u64::from(u32::MAX)) as usize;
    let end = pointer
        .checked_add(length)
        .ok_or_else(|| "this game answered outside its own memory".to_owned())?;
    memory
        .data(store)
        .get(pointer..end)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| "this game answered outside its own memory".to_owned())
}

#[cfg(test)]
mod tests;
