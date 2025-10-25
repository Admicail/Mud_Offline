use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Write};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Item {
    key: String,
    name: String,
    desc: String,
    portable: bool,
    effects: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Room {
    key: String,
    name: String,
    desc: String,
    exits: HashMap<String, String>, 
    items: Vec<String>,   suelo
    flags: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Player {
    name: String,
    location: String,
    inventory: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct World {
    rooms: IndexMap<String, Room>,
    items: IndexMap<String, Item>,
}

#[derive(Debug)]
struct Game {
    world: World,
    player: Player,
    running: bool,
}

impl Game {
    fn new(world: World) -> Self {
        Self {
            world,
            player: Player {
                name: "Hero".into(),
                location: "cave_entrance".into(),
                inventory: vec![],
            },
            running: true,
        }
    }

    fn current_room(&self) -> &Room {
        self.world.rooms.get(&self.player.location).expect("room not found")
    }

    fn current_room_mut(&mut self) -> &mut Room {
        self.world.rooms.get_mut(&self.player.location).expect("room not found")
    }

    fn find_item_here(&self, token: &str) -> Option<String> {
        let token = token.to_lowercase();
        let room = self.current_room();
        for key in &room.items {
            if let Some(it) = self.world.items.get(key) {
                if it.key.to_lowercase() == token || it.name.to_lowercase() == token {
                    return Some(it.key.clone());
                }
            }
        }
        None
    }

    fn find_item_inventory(&self, token: &str) -> Option<String> {
        let token = token.to_lowercase();
        for key in &self.player.inventory {
            if let Some(it) = self.world.items.get(key) {
                if it.key.to_lowercase() == token || it.name.to_lowercase() == token {
                    return Some(it.key.clone());
                }
            }
        }
        None
    }

    fn has_light(&self) -> bool {
        self.player.inventory.iter().any(|k| {
            self.world
                .items
                .get(k)
                .and_then(|it| it.effects.get("lights"))
                .is_some()
        })
    }

    fn cmd_look(&self) {
        let room = self.current_room();
        let is_dark = *room.flags.get("dark").unwrap_or(&false);
        let has_light = self.has_light();

        if is_dark && !has_light {
            println!("Está muy oscuro. Apenas distingues siluetas.");
            if room.exits.is_empty() {
                println!("Salidas: ninguna");
            } else {
                let exits = room.exits.keys().cloned().collect::<Vec<_>>().join(", ");
                println!("Salidas: {exits}");
            }
            return;
        }

        println!("\n{}", room.name);
        println!("{}", "-".repeat(room.name.len()));
        println!("{}", room.desc);

        if !room.items.is_empty() {
            let names: Vec<String> = room
                .items
                .iter()
                .filter_map(|k| self.world.items.get(k).map(|it| it.name.clone()))
                .collect();
            println!("\nVes aquí: {}", names.join(", "));
        }

        if room.exits.is_empty() {
            println!("Salidas: ninguna");
        } else {
            let exits = room.exits.keys().cloned().collect::<Vec<_>>().join(", ");
            println!("Salidas: {exits}");
        }
    }

    fn cmd_go(&mut self, dir: Option<&str>) {
        let Some(direction) = dir.map(|d| d.to_lowercase()) else {
            println!("Uso: go <north|south|east|west|up|down>");
            return;
        };

        let cur = self.current_room().clone();
        let Some(dest) = cur.exits.get(&direction) else {
            println!("No hay salida en esa dirección.");
            return;
        };

        // bloqueo por bandera: locked_<dir>
        let flag = format!("locked_{direction}");
        if *cur.flags.get(&flag).unwrap_or(&false) {
            // ¿tiene llave?
            let can_unlock = self.player.inventory.iter().any(|k| {
                self.world
                    .items
                    .get(k)
                    .and_then(|it| it.effects.get("unlocks"))
                    .map(|v| v == &format!("{}:{}", cur.key, direction))
                    .unwrap_or(false)
            });
            if !can_unlock {
                println!("La salida está bloqueada.");
                return;
            }
            // desbloquear
            if let Some(r) = self.world.rooms.get_mut(&cur.key) {
                r.flags.insert(flag.clone(), false);
            }
            println!("Usas la llave y desbloqueas la salida.");
        }

        self.player.location = dest.clone();
        self.cmd_look();
    }

    fn cmd_take(&mut self, tok: Option<&str>) {
        let Some(token) = tok else {
            println!("Uso: take <objeto>");
            return;
        };
        let Some(key) = self.find_item_here(token) else {
            println!("No ves eso aquí.");
            return;
        };
        let portable = self
            .world
            .items
            .get(&key)
            .map(|i| i.portable)
            .unwrap_or(false);
        if !portable {
            println!("No puedes cargar eso.");
            return;
        }
        let room = self.current_room_mut();
        if let Some(idx) = room.items.iter().position(|k| k == &key) {
            room.items.remove(idx);
        }
        self.player.inventory.push(key.clone());
        println!("Tomaste {}.", self.world.items[&key].name);
    }

    fn cmd_drop(&mut self, tok: Option<&str>) {
        let Some(token) = tok else {
            println!("Uso: drop <objeto>");
            return;
        };
        let Some(key) = self.find_item_inventory(token) else {
            println!("No llevas eso.");
            return;
        };
        if let Some(idx) = self.player.inventory.iter().position(|k| k == &key) {
            self.player.inventory.remove(idx);
        }
        self.current_room_mut().items.push(key.clone());
        println!("Dejaste {}.", self.world.items[&key].name);
    }

    fn cmd_inventory(&self) {
        if self.player.inventory.is_empty() {
            println!("No llevas nada.");
            return;
        }
        let names: Vec<String> = self
            .player
            .inventory
            .iter()
            .filter_map(|k| self.world.items.get(k).map(|it| it.name.clone()))
            .collect();
        println!("Llevas: {}", names.join(", "));
    }

    fn cmd_use(&mut self, tok: Option<&str>) {
        let Some(token) = tok else {
            println!("Uso: use <objeto>");
            return;
        };
        let Some(key) = self.find_item_inventory(token) else {
            println!("No llevas eso.");
            return;
        };
        let effects = self.world.items[&key].effects.clone();

        if effects.get("lights").is_some() {
            println!("Alzas {}. La luz revela tu entorno.", self.world.items[&key].name);
            self.cmd_look();
            return;
        }

        if let Some(tag) = effects.get("unlocks") {
            let parts: Vec<&str> = tag.split(':').collect();
            if parts.len() == 2 {
                let (rkey, dir) = (parts[0], parts[1]);
                if rkey == self.current_room().key {
                    let flag = format!("locked_{dir}");
                    if self.current_room().flags.get(&flag).copied().unwrap_or(false) {
                        if let Some(r) = self.world.rooms.get_mut(rkey) {
                            r.flags.insert(flag, false);
                        }
                        println!("Usas {} y desbloqueas la salida {}.", self.world.items[&key].name, dir);
                    } else {
                        println!("Aquí no hay nada que desbloquear.");
                    }
                } else {
                    println!("No parece servir aquí.");
                }
            } else {
                println!("La llave no está bien configurada.");
            }
            return;
        }

        println!("No pasa nada.");
    }

    fn cmd_help(&self) {
        println!(
"Comandos:
  look                 - mirar la sala
  go <dir>             - moverte (north, south, east, west, up, down)
  take <objeto>        - tomar objeto
  drop <objeto>        - soltar objeto
  use <objeto>         - usar objeto (linterna, llave, etc.)
  inv                  - inventario
  save / load          - guardar / cargar partida
  help                 - ayuda
  quit                 - salir"
        );
    }

    fn save(&self, path: &str) -> Result<()> {
        let snapshot = SaveData {
            player: self.player.clone(),
            rooms: self
                .world
                .rooms
                .iter()
                .map(|(k, r)| {
                    (
                        k.clone(),
                        RoomState {
                            items: r.items.clone(),
                            flags: r.flags.clone(),
                        },
                    )
                })
                .collect(),
        };
        let data = serde_json::to_string_pretty(&snapshot)?;
        fs::write(path, data)?;
        println!("Juego guardado en {path}");
        Ok(())
    }

    fn load(&mut self, path: &str) -> Result<()> {
        if !Path::new(path).exists() {
            return Err(anyhow!("No existe el archivo {path}"));
        }
        let data = fs::read_to_string(path)?;
        let snapshot: SaveData = serde_json::from_str(&data)?;
        self.player = snapshot.player;
        for (k, st) in snapshot.rooms {
            if let Some(r) = self.world.rooms.get_mut(&k) {
                r.items = st.items;
                r.flags = st.flags;
            }
        }
        println!("Juego cargado desde {path}");
        self.cmd_look();
        Ok(())
    }

    fn loop_run(&mut self) {
        println!("Bienvenido al mini-MUD (offline). Escribe 'help' para ver comandos.\n");
        self.cmd_look();

        while self.running {
            print!("\n> ");
            io::stdout().flush().ok();
            let mut buf = String::new();
            if io::stdin().read_line(&mut buf).is_err() {
                println!("\nSaliendo…");
                break;
            }
            let line = buf.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split_whitespace();
            let cmd = parts.next().unwrap().to_lowercase();
            let arg1 = parts.next();

            match cmd.as_str() {
                "l" | "look" => self.cmd_look(),
                "g" | "go" => self.cmd_go(arg1),
                "take" | "get" => self.cmd_take(arg1),
                "drop" => self.cmd_drop(arg1),
                "use" => self.cmd_use(arg1),
                "inv" | "inventory" => self.cmd_inventory(),
                "save" => { let _ = self.save("save.json"); }
                "load" => { if let Err(e) = self.load("save.json") { println!("{e}"); } }
                "help" => self.cmd_help(),
                "quit" | "exit" => { self.running = false; println!("¡Hasta la próxima!"); }
                _ => println!("No entiendo ese comando. Escribe 'help'."),
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RoomState {
    items: Vec<String>,
    flags: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SaveData {
    player: Player,
    rooms: HashMap<String, RoomState>,
}

fn build_world() -> World {
    // Items
    let mut items = IndexMap::new();
    items.insert(
        "torch".into(),
        Item {
            key: "torch".into(),
            name: "antorcha".into(),
            desc: "Una antorcha de madera. Aporta luz.".into(),
            portable: true,
            effects: HashMap::from([("lights".into(), "true".into())]),
        },
    );
    items.insert(
        "key_gate".into(),
        Item {
            key: "key_gate".into(),
            name: "llave vieja".into(),
            desc: "Una llave oxidada con una runa.".into(),
            portable: true,
            effects: HashMap::from([("unlocks".into(), "narrow_passage:north".into())]),
        },
    );
    items.insert(
        "note".into(),
        Item {
            key: "note".into(),
            name: "nota arrugada".into(),
            desc: "Dice: 'La luz revela lo que temes.'".into(),
            portable: true,
            effects: HashMap::new(),
        },
    );
    items.insert(
        "altar".into(),
        Item {
            key: "altar".into(),
            name: "altar de piedra".into(),
            desc: "Un altar frío y pesado. No puedes cargarlo.".into(),
            portable: false,
            effects: HashMap::new(),
        },
    );

    // Rooms
    let cave_entrance = Room {
        key: "cave_entrance".into(),
        name: "Entrada de la Cueva".into(),
        desc: "El viento helado sopla tras de ti. Un pasaje oscuro se interna hacia el norte.".into(),
        exits: HashMap::from([("north".into(), "narrow_passage".into())]),
        items: vec!["note".into(), "torch".into()],
        flags: HashMap::new(),
    };
    let narrow_passage = Room {
        key: "narrow_passage".into(),
        name: "Pasadizo Estrecho".into(),
        desc: "Las paredes se cierran a tu alrededor. Al norte ves una reja; al sur, la salida.".into(),
        exits: HashMap::from([
            ("south".into(), "cave_entrance".into()),
            ("north".into(), "ancient_chamber".into()),
        ]),
        items: vec!["key_gate".into()],
        flags: HashMap::from([("dark".into(), true), ("locked_north".into(), true)]),
    };
    let ancient_chamber = Room {
        key: "ancient_chamber".into(),
        name: "Cámara Ancestral".into(),
        desc: "Una sala amplia con grabados antiguos. Un altar domina el centro.".into(),
        exits: HashMap::from([("south".into(), "narrow_passage".into())]),
        items: vec!["altar".into()],
        flags: HashMap::new(),
    };

    let mut rooms = IndexMap::new();
    rooms.insert(cave_entrance.key.clone(), cave_entrance);
    rooms.insert(narrow_passage.key.clone(), narrow_passage);
    rooms.insert(ancient_chamber.key.clone(), ancient_chamber);

    World { rooms, items }
}

fn main() {
    let world = build_world();
    let mut game = Game::new(world);
    game.loop_run();
}

