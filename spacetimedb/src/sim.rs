use serde::Serialize;
use std::collections::HashMap;

pub const LIMIT: u32 = 2400;

#[derive(Clone)]
enum Word {
    Value(i32),
    Ref(String),
    Read(String),
    Op(String),
}

#[derive(Clone)]
pub struct Program {
    words: Vec<Word>,
    labels: HashMap<String, i32>,
}

pub fn compile(source: &str) -> Result<Program, String> {
    let mut program = Program {
        words: Vec::new(),
        labels: HashMap::new(),
    };
    let mut comment = false;
    for line in source.split(['\r', '\n']) {
        let mut code = String::new();
        for ch in line.chars() {
            match ch {
                '{' => {
                    comment = true;
                    code.push(' ');
                }
                '}' => {
                    comment = false;
                    code.push(' ');
                }
                '#' | ';' if !comment => break,
                _ if !comment => code.push(ch),
                _ => {}
            }
        }
        for token in code
            .replace(':', ": ")
            .to_ascii_uppercase()
            .split_whitespace()
        {
            let word = if let Some(label) = token.strip_suffix(':') {
                if label.is_empty()
                    || program
                        .labels
                        .insert(label.into(), program.words.len() as i32)
                        .is_some()
                {
                    return Err(format!("Invalid or duplicate label: {token}"));
                }
                continue;
            } else if let Ok(value) = token.parse::<i32>() {
                Word::Value(value)
            } else if let Some(register) = token.strip_suffix('\'') {
                Word::Ref(register.into())
            } else if OPS.split_ascii_whitespace().any(|op| op == token) {
                Word::Op(token.into())
            } else {
                Word::Read(token.into())
            };
            program.words.push(word);
        }
    }
    if comment || program.words.is_empty() || program.words.len() > 5000 {
        return Err("Expected 1–5000 instructions and balanced comments".into());
    }
    for word in &program.words {
        match word {
            Word::Read(name) if !program.labels.contains_key(name) && !register(name) => {
                return Err(format!("Unknown word: {name}"));
            }
            Word::Ref(name) if !register(name) => return Err(format!("Unknown register: {name}")),
            _ => {}
        }
    }
    Ok(program)
}

const OPS: &str = "+ - * / = ! < > ABS AND OR XOR EOR CHS MAX MIN MOD NOT SQRT SIN SINE COS COSINE TAN TANGENT ARCSIN ARCCOS ARCTAN DIST STO STORE RECALL VSTORE VRECALL IF IFE IFG IFEG CALL JUMP RETURN NOP SYNC DROP DUP DUPLICATE DROPALL SWAP ROLL INTON INTOFF FLUSHINT RTI SETINT SETPARAM DEBUG DEBUGGER BEEP PRINT";
const REGISTERS: &str = "AIM BULLET BOTTOM BOT CHANNEL CHRONON COLLISION DAMAGE DOPPLER ENERGY FIRE FRIEND HISTORY HELL HELLBORE LASER ID KILLS LEFT LOOK MINE MISSILE MOVEX MOVEY NUKE PROBE RADAR RANDOM RANGE RIGHT ROBOTS SCAN SHIELD SIGNAL SPEEDX SPEEDY STUNNER TEAMMATES TOP WALL X Y";

fn register(name: &str) -> bool {
    (name.len() == 1 && name.as_bytes()[0].is_ascii_uppercase())
        || REGISTERS.split_ascii_whitespace().any(|word| word == name)
        || (name.len() == 5
            && (name.starts_with("ICON") || name.starts_with("SND"))
            && name.as_bytes()[4].is_ascii_digit())
        || (name.len() == 4 && name.starts_with("SND") && name.as_bytes()[3].is_ascii_digit())
}

#[derive(Clone)]
enum Item {
    Number(i32),
    Ref(String),
}

#[derive(Clone, Serialize)]
struct Body {
    x: f64,
    y: f64,
    aim: i32,
    hp: i32,
    energy: i32,
    shield: i32,
}

struct Bot {
    program: Program,
    body: Body,
    pc: usize,
    stack: Vec<Item>,
    registers: HashMap<String, i32>,
    interrupts: Vec<(String, i32)>,
    params: HashMap<String, i32>,
    vector: HashMap<i32, i32>,
    enabled: bool,
    collision: bool,
    stasis: u32,
}

#[derive(Clone, Serialize)]
struct Shot {
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    owner: usize,
    power: i32,
    kind: String,
    life: u32,
}

#[derive(Serialize)]
pub struct Frame {
    tick: u32,
    bots: Vec<Body>,
    shots: Vec<[f64; 2]>,
}

pub struct Outcome {
    pub winner: Option<usize>,
    pub frames: Vec<Frame>,
}

struct Sensors<'a> {
    tick: u32,
    index: usize,
    bots: &'a [Body],
    shots: &'a [Shot],
}

fn direction(aim: i32) -> (f64, f64) {
    let radians = f64::from(aim).to_radians();
    (radians.sin(), -radians.cos())
}

impl Bot {
    fn read(&self, name: &str, sensors: &Sensors) -> i32 {
        let body = &self.body;
        let (dx, dy) = direction(body.aim + self.reg("SCAN"));
        let target = sensors
            .bots
            .iter()
            .enumerate()
            .filter(|(index, other)| *index != sensors.index && other.hp > 0)
            .filter_map(|(_, other)| {
                let x = other.x - body.x;
                let y = other.y - body.y;
                let distance = x * dx + y * dy;
                (distance > 0.0 && (x * dy - y * dx).abs() < 8.0)
                    .then_some((distance as i32, other))
            })
            .min_by_key(|(distance, _)| *distance);
        match name {
            "X" | "LEFT" | "RIGHT" => body.x as i32,
            "Y" | "TOP" | "BOTTOM" | "BOT" => body.y as i32,
            "AIM" => body.aim,
            "ENERGY" => body.energy,
            "DAMAGE" => body.hp,
            "SHIELD" => body.shield,
            "CHRONON" => sensors.tick as i32,
            "COLLISION" => i32::from(self.collision),
            "WALL" => {
                i32::from(body.x <= 8.0 || body.x >= 292.0 || body.y <= 8.0 || body.y >= 292.0)
            }
            "ID" => sensors.index as i32 + 1,
            "ROBOTS" => sensors.bots.iter().filter(|other| other.hp > 0).count() as i32,
            "RANGE" => target.map_or(0, |(distance, _)| distance),
            "PROBE" => target.map_or(0, |(_, other)| match self.reg("PROBE") {
                1 => other.energy,
                2 => other.shield,
                3 => other.aim,
                _ => other.hp,
            }),
            "RADAR" => sensors
                .shots
                .iter()
                .filter(|shot| shot.owner != sensors.index)
                .filter_map(|shot| {
                    let x = shot.x - body.x;
                    let y = shot.y - body.y;
                    let distance = x * dx + y * dy;
                    (distance > 0.0 && (x * dy - y * dx).abs() < 18.0).then_some(distance as i32)
                })
                .min()
                .unwrap_or(0),
            _ => self.reg(name),
        }
    }

    fn reg(&self, name: &str) -> i32 {
        self.registers.get(name).copied().unwrap_or(0)
    }

    fn pop(&mut self) -> Result<i32, ()> {
        match self.stack.pop() {
            Some(Item::Number(value)) => Ok(value),
            _ => Err(()),
        }
    }

    fn reference(&mut self) -> Result<String, ()> {
        match self.stack.pop() {
            Some(Item::Ref(name)) => Ok(name),
            _ => Err(()),
        }
    }

    fn push(&mut self, value: i32) {
        self.stack.push(Item::Number(value));
    }

    fn jump(&mut self, address: i32, call: bool) -> Result<(), ()> {
        if address < 0 || address as usize >= self.program.words.len() {
            return Err(());
        }
        if call {
            self.push(self.pc as i32);
        }
        self.pc = address as usize;
        Ok(())
    }

    fn write(&mut self, name: String, value: i32, owner: usize, shots: &mut Vec<Shot>) {
        let body = &mut self.body;
        match name.as_str() {
            "AIM" => body.aim = value.rem_euclid(360),
            "SHIELD" => {
                let increase = (value.clamp(0, 30) - body.shield).clamp(0, body.energy);
                body.shield += increase;
                body.energy -= increase;
            }
            "MOVEX" | "MOVEY" => {
                let movement = value.clamp(-20, 20);
                if body.energy >= movement.abs() {
                    body.energy -= movement.abs();
                    if name == "MOVEX" {
                        body.x += f64::from(movement) / 2.0;
                    } else {
                        body.y += f64::from(movement) / 2.0;
                    }
                }
            }
            "FIRE" | "BULLET" | "MISSILE" | "NUKE" | "MINE" | "STUNNER" | "HELL" | "HELLBORE"
            | "LASER" => {
                let power = value.clamp(0, body.energy.min(150));
                if power == 0 || shots.len() >= 256 {
                    return;
                }
                body.energy -= power;
                let (dx, dy) = direction(body.aim);
                let speed = if name == "MINE" { 0.0 } else { 7.0 };
                shots.push(Shot {
                    x: body.x + 10.0 * dx,
                    y: body.y + 10.0 * dy,
                    dx: speed * dx,
                    dy: speed * dy,
                    owner,
                    power,
                    life: if name == "MINE" { 200 } else { 60 },
                    kind: name,
                });
            }
            "SPEEDX" | "SPEEDY" => {
                let value = value.clamp(-12, 12);
                let cost = (value - self.reg(&name)).abs();
                if self.body.energy >= cost {
                    self.body.energy -= cost;
                    self.registers.insert(name, value);
                }
            }
            _ => {
                self.registers.insert(name, value);
            }
        }
    }

    fn execute(&mut self, op: &str, sensors: &Sensors, shots: &mut Vec<Shot>) -> Result<bool, ()> {
        match op {
            "+" | "-" | "*" | "/" | "MOD" | "=" | "!" | ">" | "<" | "AND" | "OR" | "XOR"
            | "EOR" | "MAX" | "MIN" => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.push(match op {
                    "+" => a.wrapping_add(b),
                    "-" => a.wrapping_sub(b),
                    "*" => a.wrapping_mul(b),
                    "/" => a.checked_div(b).ok_or(())?,
                    "MOD" => a.checked_rem(b).ok_or(())?,
                    "=" => i32::from(a == b),
                    "!" => i32::from(a != b),
                    ">" => i32::from(a > b),
                    "<" => i32::from(a < b),
                    "AND" => i32::from(a != 0 && b != 0),
                    "OR" => i32::from(a != 0 || b != 0),
                    "XOR" | "EOR" => i32::from((a != 0) ^ (b != 0)),
                    "MAX" => a.max(b),
                    _ => a.min(b),
                });
            }
            "ABS" | "CHS" | "NOT" | "SQRT" => {
                let value = self.pop()?;
                self.push(match op {
                    "ABS" => value.saturating_abs(),
                    "CHS" => value.saturating_neg(),
                    "NOT" => i32::from(value == 0),
                    _ => f64::from(value).sqrt() as i32,
                });
            }
            "SIN" | "SINE" | "COS" | "COSINE" | "TAN" | "TANGENT" => {
                let angle = f64::from(self.pop()?).to_radians();
                let magnitude = f64::from(self.pop()?);
                let factor = match op {
                    "SIN" | "SINE" => angle.sin(),
                    "COS" | "COSINE" => angle.cos(),
                    _ => angle.tan(),
                };
                self.push((magnitude * factor) as i32);
            }
            "ARCSIN" | "ARCCOS" => {
                let magnitude = f64::from(self.pop()?);
                let value = f64::from(self.pop()?) / magnitude;
                self.push(
                    if op == "ARCSIN" {
                        value.asin()
                    } else {
                        value.acos()
                    }
                    .to_degrees() as i32,
                );
            }
            "ARCTAN" => {
                let y = f64::from(self.pop()?);
                let x = f64::from(self.pop()?);
                self.push(((450.5 - (-y).atan2(x).to_degrees()) as i32).rem_euclid(360));
            }
            "DIST" => {
                let y = f64::from(self.pop()?);
                let x = f64::from(self.pop()?);
                self.push((self.body.x - x).hypot(self.body.y - y) as i32);
            }
            "STORE" | "STO" => {
                let name = self.reference()?;
                let value = self.pop()?;
                self.write(name, value, sensors.index, shots);
            }
            "RECALL" => {
                let name = self.reference()?;
                self.push(self.read(&name, sensors));
            }
            "VSTORE" => {
                let index = self.pop()?;
                let value = self.pop()?;
                if (0..=100).contains(&index) {
                    self.vector.insert(index, value);
                }
            }
            "VRECALL" => {
                let index = self.pop()?;
                self.push(self.vector.get(&index).copied().unwrap_or(0));
            }
            "IF" | "IFG" | "IFE" | "IFEG" => {
                let last = self.pop()?;
                let alternate = op == "IFE" || op == "IFEG";
                let yes = if alternate { self.pop()? } else { last };
                if self.pop()? != 0 {
                    self.jump(yes, op == "IF" || op == "IFE")?;
                } else if alternate {
                    self.jump(last, op == "IFE")?;
                }
            }
            "CALL" | "JUMP" | "RETURN" | "RTI" => {
                let address = self.pop()?;
                self.jump(address, op == "CALL")?;
                if op == "RTI" {
                    self.enabled = true;
                }
            }
            "SETINT" => {
                let name = self.reference()?;
                let address = self.pop()?;
                self.interrupts.retain(|(key, _)| key != &name);
                self.interrupts.push((name, address));
            }
            "SETPARAM" => {
                let name = self.reference()?;
                let value = match self.stack.pop().ok_or(())? {
                    Item::Number(value) => value,
                    Item::Ref(value) => match value.as_str() {
                        "ENERGY" => 1,
                        "SHIELD" => 2,
                        "AIM" => 3,
                        _ => 0,
                    },
                };
                self.params.insert(name.clone(), value);
                if name == "PROBE" {
                    self.registers.insert(name, value);
                }
            }
            "INTON" => self.enabled = true,
            "INTOFF" => self.enabled = false,
            "DROP" => {
                self.stack.pop().ok_or(())?;
            }
            "DROPALL" => self.stack.clear(),
            "DUP" | "DUPLICATE" => self.stack.push(self.stack.last().cloned().ok_or(())?),
            "SWAP" => {
                let len = self.stack.len();
                if len < 2 {
                    return Err(());
                }
                self.stack.swap(len - 1, len - 2);
            }
            "ROLL" => {
                let depth = self.pop()? as usize;
                let index = self.stack.len().checked_sub(depth).ok_or(())?;
                if index >= self.stack.len() {
                    return Err(());
                }
                let item = self.stack.remove(index);
                self.stack.push(item);
            }
            "SYNC" => return Ok(true),
            _ => {}
        }
        Ok(false)
    }

    fn run(
        &mut self,
        sensors: &Sensors,
        shots: &mut Vec<Shot>,
        random: &mut u64,
    ) -> Result<(), ()> {
        if self.stasis > 0 {
            self.stasis -= 1;
            return Ok(());
        }
        self.body.energy = (self.body.energy + 2).min(150);
        if sensors.tick % 2 == 0 {
            self.body.shield = (self.body.shield - 1).max(0);
        }
        if self.enabled {
            if let Some((_, address)) = self.interrupts.iter().find(|(name, _)| {
                let value = self.read(name, sensors);
                let parameter = self.params.get(name).copied();
                match name.as_str() {
                    "LEFT" | "TOP" => value < parameter.unwrap_or(20),
                    "RIGHT" | "BOTTOM" | "BOT" => value > parameter.unwrap_or(280),
                    "RANGE" | "RADAR" => value > 0 && value < parameter.unwrap_or(600),
                    "CHRONON" => value == parameter.unwrap_or(-1),
                    _ => value != 0,
                }
            }) {
                self.jump(*address, true)?;
                self.enabled = false;
            }
        }
        for _ in 0..15 {
            if self.pc >= self.program.words.len() {
                self.pc = 0;
            }
            let word = self.program.words[self.pc].clone();
            self.pc += 1;
            match word {
                Word::Value(value) => self.push(value),
                Word::Ref(name) => self.stack.push(Item::Ref(name)),
                Word::Read(name) if name == "RANDOM" => {
                    *random ^= *random << 13;
                    *random ^= *random >> 7;
                    *random ^= *random << 17;
                    self.push((*random % 360) as i32);
                }
                Word::Read(name) => self.push(
                    self.program
                        .labels
                        .get(&name)
                        .copied()
                        .unwrap_or_else(|| self.read(&name, sensors)),
                ),
                Word::Op(op) => {
                    if self.execute(&op, sensors, shots)? {
                        break;
                    }
                }
            }
            if self.stack.len() > 256 {
                return Err(());
            }
        }
        Ok(())
    }
}

pub fn simulate(programs: Vec<Program>, seed: u64, replay: bool) -> Outcome {
    let count = programs.len();
    let mut random = seed.max(1);
    let phase = (seed % 360) as f64;
    let mut bots: Vec<_> = programs
        .into_iter()
        .enumerate()
        .map(|(index, program)| {
            let angle = phase + 360.0 * index as f64 / count as f64;
            let (dx, dy) = direction(angle as i32);
            Bot {
                program,
                body: Body {
                    x: 150.0 + 92.0 * dx,
                    y: 150.0 + 92.0 * dy,
                    aim: (angle as i32 + 180) % 360,
                    hp: 100,
                    energy: 150,
                    shield: 0,
                },
                pc: 0,
                stack: Vec::new(),
                registers: HashMap::new(),
                interrupts: Vec::new(),
                params: HashMap::new(),
                vector: HashMap::new(),
                enabled: false,
                collision: false,
                stasis: 0,
            }
        })
        .collect();
    let mut shots: Vec<Shot> = Vec::new();
    let mut frames = Vec::new();
    let mut winner = None;
    for tick in 0..=LIMIT {
        let alive: Vec<_> = bots
            .iter()
            .enumerate()
            .filter(|(_, bot)| bot.body.hp > 0)
            .map(|(i, _)| i)
            .collect();
        if replay && (tick % 6 == 0 || alive.len() <= 1) {
            frames.push(Frame {
                tick,
                bots: bots.iter().map(|bot| bot.body.clone()).collect(),
                shots: shots.iter().take(64).map(|shot| [shot.x, shot.y]).collect(),
            });
        }
        if alive.len() <= 1 {
            winner = alive.first().copied();
            break;
        }
        let bodies: Vec<_> = bots.iter().map(|bot| bot.body.clone()).collect();
        let old_shots = shots.clone();
        for &index in &alive {
            let bot = &mut bots[index];
            let sensors = Sensors {
                tick,
                index,
                bots: &bodies,
                shots: &old_shots,
            };
            if bot.run(&sensors, &mut shots, &mut random).is_err() {
                bot.body.hp = 0;
            }
            bot.collision = false;
            let x = bot.body.x + f64::from(bot.reg("SPEEDX"));
            let y = bot.body.y + f64::from(bot.reg("SPEEDY"));
            bot.body.x = x.clamp(8.0, 292.0);
            bot.body.y = y.clamp(8.0, 292.0);
            if x != bot.body.x || y != bot.body.y {
                bot.body.hp -= 3;
            }
        }
        for (offset, &first) in alive.iter().enumerate() {
            for &second in &alive[offset + 1..] {
                let a = &bots[first].body;
                let b = &bots[second].body;
                if a.hp <= 0 || b.hp <= 0 || (a.x - b.x).hypot(a.y - b.y) > 16.0 {
                    continue;
                }
                for index in [first, second] {
                    let bot = &mut bots[index];
                    bot.body.x = bodies[index].x;
                    bot.body.y = bodies[index].y;
                    bot.body.hp -= 1;
                    bot.collision = true;
                    bot.registers.insert("SPEEDX".into(), 0);
                    bot.registers.insert("SPEEDY".into(), 0);
                }
            }
        }
        shots.retain_mut(|shot| {
            shot.x += shot.dx;
            shot.y += shot.dy;
            shot.life -= 1;
            if shot.life == 0
                || !(0.0..=300.0).contains(&shot.x)
                || !(0.0..=300.0).contains(&shot.y)
            {
                return false;
            }
            for (index, bot) in bots.iter_mut().enumerate() {
                let body = &mut bot.body;
                if index == shot.owner
                    || body.hp <= 0
                    || (body.x - shot.x).hypot(body.y - shot.y) > 9.0
                {
                    continue;
                }
                match shot.kind.as_str() {
                    "STUNNER" => bot.stasis += shot.power as u32 / 4,
                    "HELL" | "HELLBORE" => body.shield = 0,
                    _ => {
                        let blocked = shot.power.min(body.shield);
                        body.shield -= blocked;
                        body.hp -= shot.power - blocked;
                    }
                }
                return false;
            }
            true
        });
    }
    Outcome { winner, frames }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archived_programs_compile() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../legacy");
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let source = std::fs::read_to_string(&path).unwrap();
            assert!(
                compile(&source).is_ok(),
                "{}: {:?}",
                path.display(),
                compile(&source).err()
            );
        }
    }

    #[test]
    fn shooter_defeats_idle_robot_and_replay_is_emitted() {
        let shooter = compile("LOOP: 20 FIRE' STORE SYNC LOOP JUMP").unwrap();
        let idle = compile("LOOP: SYNC LOOP JUMP").unwrap();
        let result = simulate(vec![shooter, idle], 180, true);
        assert_eq!(result.winner, Some(0));
        assert_eq!(result.frames[0].tick, 0);
        assert!(result.frames.last().unwrap().bots[1].hp <= 0);
    }
}
