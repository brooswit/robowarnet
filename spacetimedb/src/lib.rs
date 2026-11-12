mod sim;

use spacetimedb::{reducer, table, view, Identity, ReducerContext, ScheduleAt, Table, ViewContext};
use std::time::Duration;

#[table(accessor = account, public)]
pub struct Account {
    #[primary_key]
    identity: Identity,
    #[unique]
    handle: String,
}

#[table(accessor = robot, public)]
#[derive(Clone)]
pub struct Robot {
    #[primary_key]
    #[auto_inc]
    id: u64,
    #[index(btree)]
    owner: Identity,
    name: String,
    rank: u64,
    bouts: u64,
}

#[table(accessor = robot_source)]
pub struct RobotSource {
    #[primary_key]
    id: u64,
    #[index(btree)]
    owner: Identity,
    source: String,
}

#[view(accessor = my_sources, public)]
pub fn my_sources(ctx: &ViewContext) -> Vec<RobotSource> {
    ctx.db.robot_source().owner().filter(ctx.sender()).collect()
}

#[table(accessor = lobby, public)]
pub struct Lobby {
    #[primary_key]
    #[auto_inc]
    id: u64,
    host: Identity,
    name: String,
}

#[table(accessor = seat, public)]
pub struct Seat {
    #[primary_key]
    identity: Identity,
    #[index(btree)]
    lobby: u64,
    robot: Option<u64>,
}

#[table(accessor = replay, public)]
pub struct Replay {
    #[primary_key]
    lobby: u64,
    names: Vec<String>,
    winner: Option<u8>,
    frames: String,
    started: i64,
}

#[table(accessor = ladder, public)]
pub struct Ladder {
    #[primary_key]
    id: u8,
    sweep: u64,
    cursor: u64,
    lower: String,
    higher: String,
    winner: String,
}

#[table(accessor = ladder_timer, scheduled(run_ladder))]
pub struct LadderTimer {
    #[primary_key]
    #[auto_inc]
    scheduled_id: u64,
    scheduled_at: ScheduleAt,
}

const LEGACY: Identity = Identity::ZERO;
const MAX_SEATS: usize = 6;

fn require_account(ctx: &ReducerContext) -> Result<(), String> {
    if ctx.db.account().identity().find(ctx.sender()).is_none() {
        return Err("Choose a handle first.".into());
    }
    Ok(())
}

fn title(name: String) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
        return Err("Use a name of 1–40 characters.".into());
    }
    Ok(name.into())
}

#[reducer(init)]
pub fn init(ctx: &ReducerContext) {
    ctx.db.account().insert(Account {
        identity: LEGACY,
        handle: "legacy".into(),
    });
    ctx.db.ladder().insert(Ladder {
        id: 0,
        sweep: 1,
        cursor: 0,
        lower: String::new(),
        higher: String::new(),
        winner: String::new(),
    });
    let bots = [
        ("Arachnee", include_str!("../../legacy/arachnee.txt")),
        ("Archivist", include_str!("../../legacy/archivist.txt")),
        (
            "Corner Hopper",
            include_str!("../../legacy/corner-hopper.txt"),
        ),
        (
            "Dark Knight 4",
            include_str!("../../legacy/dark-knight-4.txt"),
        ),
        (
            "Dgt With Probes",
            include_str!("../../legacy/dgt-with-probes.txt"),
        ),
        (
            "Existentialist",
            include_str!("../../legacy/existentialist.txt"),
        ),
        ("Fluffy 3", include_str!("../../legacy/fluffy-3.txt")),
        ("Ghost", include_str!("../../legacy/ghost.txt")),
        (
            "Invisible Stalker",
            include_str!("../../legacy/invisible-stalker.txt"),
        ),
        ("Locke", include_str!("../../legacy/locke.txt")),
        ("Pearl", include_str!("../../legacy/pearl.txt")),
        (
            "Shape Changer",
            include_str!("../../legacy/shape-changer.txt"),
        ),
        ("Silo Iv", include_str!("../../legacy/silo-iv.txt")),
        (
            "Soul Deliverer 9X",
            include_str!("../../legacy/soul-deliverer-9x.txt"),
        ),
        ("Stationary", include_str!("../../legacy/stationary.txt")),
        (
            "The Dead Parrot",
            include_str!("../../legacy/the-dead-parrot.txt"),
        ),
        ("Timbot Iv", include_str!("../../legacy/timbot-iv.txt")),
        ("Wall Hugger", include_str!("../../legacy/wall-hugger.txt")),
        ("Wall Seeker", include_str!("../../legacy/wall-seeker.txt")),
        ("Wanderer", include_str!("../../legacy/wanderer.txt")),
        ("Zim", include_str!("../../legacy/zim.txt")),
    ];
    for (rank, (name, source)) in bots.into_iter().enumerate() {
        insert_robot(ctx, LEGACY, name.into(), source.into(), rank as u64);
    }
    ctx.db.ladder_timer().insert(LadderTimer {
        scheduled_id: 0,
        scheduled_at: Duration::from_millis(250).into(),
    });
}

#[reducer]
pub fn register(ctx: &ReducerContext, handle: String) -> Result<(), String> {
    let handle = handle.trim().to_ascii_lowercase();
    if !(3..=24).contains(&handle.len())
        || !handle
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == b'_' || ch == b'-')
    {
        return Err("Use 3–24 letters, numbers, underscores or hyphens.".into());
    }
    if ctx.db.account().identity().find(ctx.sender()).is_some()
        || ctx.db.account().handle().find(&handle).is_some()
    {
        return Err("The identity already has a handle or the handle is taken.".into());
    }
    ctx.db.account().insert(Account {
        identity: ctx.sender(),
        handle,
    });
    Ok(())
}

fn insert_robot(ctx: &ReducerContext, owner: Identity, name: String, source: String, rank: u64) {
    let robot = ctx.db.robot().insert(Robot {
        id: 0,
        owner,
        name,
        rank,
        bouts: 0,
    });
    ctx.db.robot_source().insert(RobotSource {
        id: robot.id,
        owner,
        source,
    });
}

fn enter_at_bottom(ctx: &ReducerContext, previous_rank: Option<u64>) {
    for mut robot in ctx.db.robot().iter() {
        if previous_rank.is_none_or(|rank| robot.rank < rank) {
            robot.rank += 1;
            ctx.db.robot().id().update(robot);
        }
    }
    if let Some(mut ladder) = ctx.db.ladder().id().find(0) {
        ladder.cursor = 0;
        ctx.db.ladder().id().update(ladder);
    }
}

#[reducer]
pub fn save_robot(
    ctx: &ReducerContext,
    id: Option<u64>,
    name: String,
    source: String,
) -> Result<(), String> {
    require_account(ctx)?;
    let name = title(name)?;
    if source.len() > 32_768 {
        return Err("Source exceeds 32 KiB.".into());
    }
    sim::compile(&source)?;
    if let Some(id) = id {
        let mut robot = ctx
            .db
            .robot()
            .id()
            .find(id)
            .filter(|row| row.owner == ctx.sender())
            .ok_or("Robot not found.")?;
        let mut private = ctx
            .db
            .robot_source()
            .id()
            .find(id)
            .ok_or("Source not found.")?;
        if private.source != source {
            enter_at_bottom(ctx, Some(robot.rank));
            robot.rank = 0;
            robot.bouts = 0;
            private.source = source;
            ctx.db.robot_source().id().update(private);
        }
        robot.name = name;
        ctx.db.robot().id().update(robot);
    } else {
        if ctx.db.robot().owner().filter(ctx.sender()).count() >= 25 {
            return Err("Your robot bay is full.".into());
        }
        enter_at_bottom(ctx, None);
        insert_robot(ctx, ctx.sender(), name, source, 0);
    }
    Ok(())
}

fn leave(ctx: &ReducerContext) {
    let Some(seat) = ctx.db.seat().identity().find(ctx.sender()) else {
        return;
    };
    ctx.db.seat().identity().delete(ctx.sender());
    let Some(mut lobby) = ctx.db.lobby().id().find(seat.lobby) else {
        return;
    };
    if lobby.host != ctx.sender() {
        return;
    }
    if let Some(next) = ctx
        .db
        .seat()
        .lobby()
        .filter(lobby.id)
        .min_by_key(|seat| seat.identity)
    {
        lobby.host = next.identity;
        ctx.db.lobby().id().update(lobby);
    } else {
        ctx.db.lobby().id().delete(lobby.id);
        ctx.db.replay().lobby().delete(lobby.id);
    }
}

#[reducer]
pub fn create_lobby(ctx: &ReducerContext, name: String) -> Result<(), String> {
    require_account(ctx)?;
    let name = title(name)?;
    leave(ctx);
    let lobby = ctx.db.lobby().insert(Lobby {
        id: 0,
        host: ctx.sender(),
        name,
    });
    ctx.db.seat().insert(Seat {
        identity: ctx.sender(),
        lobby: lobby.id,
        robot: None,
    });
    Ok(())
}

#[reducer]
pub fn join_lobby(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    require_account(ctx)?;
    if ctx
        .db
        .seat()
        .identity()
        .find(ctx.sender())
        .is_some_and(|seat| seat.lobby == id)
    {
        return Ok(());
    }
    if ctx.db.lobby().id().find(id).is_none() {
        return Err("Lobby no longer exists.".into());
    }
    if ctx.db.seat().lobby().filter(id).count() >= MAX_SEATS {
        return Err("Lobby is full.".into());
    }
    leave(ctx);
    ctx.db.seat().insert(Seat {
        identity: ctx.sender(),
        lobby: id,
        robot: None,
    });
    Ok(())
}

#[reducer]
pub fn leave_lobby(ctx: &ReducerContext) {
    leave(ctx);
}

#[reducer]
pub fn choose_robot(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    let mut seat = ctx
        .db
        .seat()
        .identity()
        .find(ctx.sender())
        .ok_or("Join a lobby first.")?;
    ctx.db
        .robot()
        .id()
        .find(id)
        .filter(|row| row.owner == ctx.sender())
        .ok_or("Choose one of your robots.")?;
    seat.robot = Some(id);
    ctx.db.seat().identity().update(seat);
    Ok(())
}

fn program(ctx: &ReducerContext, id: u64) -> Result<sim::Program, String> {
    let row = ctx
        .db
        .robot_source()
        .id()
        .find(id)
        .ok_or("Source not found.")?;
    // Do not return another player's source diagnostics from match reducers.
    sim::compile(&row.source).map_err(|_| "A selected robot could not compile.".into())
}

#[reducer]
pub fn start_match(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    let lobby = ctx
        .db
        .lobby()
        .id()
        .find(id)
        .filter(|row| row.host == ctx.sender())
        .ok_or("Only the host can start a match.")?;
    let now = ctx.timestamp.to_micros_since_unix_epoch();
    if ctx
        .db
        .replay()
        .lobby()
        .find(id)
        .is_some_and(|row| now - row.started < 2_000_000)
    {
        return Err("Wait before starting another match.".into());
    }
    let mut seats: Vec<_> = ctx.db.seat().lobby().filter(id).collect();
    seats.sort_by_key(|seat| seat.identity);
    if seats.len() < 2 {
        return Err("At least two players are required.".into());
    }
    let mut names = Vec::new();
    let mut programs = Vec::new();
    for seat in seats {
        let id = seat.robot.ok_or("Every player must choose a robot.")?;
        let robot = ctx
            .db
            .robot()
            .id()
            .find(id)
            .filter(|row| row.owner == seat.identity)
            .ok_or("A robot is no longer available.")?;
        names.push(robot.name);
        programs.push(program(ctx, id)?);
    }
    let result = sim::simulate(programs, now as u64, true);
    let frames =
        serde_json::to_string(&result.frames).map_err(|_| "Could not encode match replay.")?;
    ctx.db.replay().lobby().delete(lobby.id);
    ctx.db.replay().insert(Replay {
        lobby: lobby.id,
        names,
        winner: result.winner.map(|index| index as u8),
        frames,
        started: now,
    });
    Ok(())
}

#[reducer]
pub fn run_ladder(ctx: &ReducerContext, _timer: LadderTimer) {
    let Some(mut ladder) = ctx.db.ladder().id().find(0) else {
        return;
    };
    let mut robots: Vec<_> = ctx.db.robot().iter().collect();
    robots.sort_by_key(|robot| robot.rank);
    let index = ladder.cursor as usize;
    if index + 1 >= robots.len() {
        return;
    }
    let (left, right) = robots.split_at_mut(index + 1);
    let lower = &mut left[index];
    let higher = &mut right[0];
    let programs = [program(ctx, lower.id), program(ctx, higher.id)];
    let winner = match programs {
        [Ok(a), Ok(b)] => {
            sim::simulate(
                vec![a, b],
                ctx.timestamp.to_micros_since_unix_epoch() as u64,
                false,
            )
            .winner
        }
        [Ok(_), Err(_)] => Some(0),
        [Err(_), Ok(_)] => Some(1),
        _ => None,
    };
    ladder.lower = lower.name.clone();
    ladder.higher = higher.name.clone();
    ladder.winner = match winner {
        Some(0) => lower.name.clone(),
        Some(1) => higher.name.clone(),
        _ => "Draw".into(),
    };
    if winner == Some(0) {
        std::mem::swap(&mut lower.rank, &mut higher.rank);
    }
    lower.bouts += 1;
    higher.bouts += 1;
    ctx.db.robot().id().update(lower.clone());
    ctx.db.robot().id().update(higher.clone());
    ladder.cursor += 1;
    if ladder.cursor as usize + 1 == robots.len() {
        ladder.cursor = 0;
        ladder.sweep += 1;
    }
    ctx.db.ladder().id().update(ladder);
}
