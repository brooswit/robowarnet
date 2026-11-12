import React, { useEffect, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { SpacetimeDBProvider, useSpacetimeDB, useTable } from 'spacetimedb/react';
import { DbConnection, tables } from './bindings';
import './style.css';

const uri = import.meta.env.VITE_STDB_URL || 'ws://127.0.0.1:3000';
const database = import.meta.env.VITE_STDB_DATABASE || 'robowarnet';
const tokenKey = `robowarnet:${uri}:${database}`;
const colors = ['#eaff72', '#69dfef', '#ff946c', '#c7aaff', '#ffa6cb', '#a1f8bd', '#ffcf66', '#adbbff', '#e5a9ff', '#ffffff'];
const initialSource = `{ Sweep the arena and shoot when RANGE sees a target. }
Main:
  RANGE 0 > Shoot IF
  AIM 9 + AIM' STORE
  SYNC
  Main JUMP

Shoot:
  16 FIRE' STORE
  RETURN
`;

const builder = DbConnection.builder()
  .withUri(uri)
  .withDatabaseName(database)
  .withToken(localStorage.getItem(tokenKey) || undefined)
  .onConnect((_connection, _identity, token) => localStorage.setItem(tokenKey, token));

const same = (left, right) => Boolean(left && right && left.isEqual(right));

function Arena({ replay, title = 'Arena' }) {
  const frames = useMemo(() => replay ? JSON.parse(replay.frames) : [], [replay?.started]);
  const [position, setPosition] = useState(0);
  const [playing, setPlaying] = useState(false);

  useEffect(() => {
    setPosition(0);
    setPlaying(frames.length > 1);
  }, [frames]);

  useEffect(() => {
    if (!playing || position >= frames.length - 1) return;
    const timer = setTimeout(() => setPosition(position + 1), 40);
    return () => clearTimeout(timer);
  }, [playing, position, frames]);

  const frame = frames[Math.min(position, frames.length - 1)];
  const finished = Boolean(frame && position === frames.length - 1);
  return <section className="panel arena">
    <div className="panel-heading"><h2>{title}</h2><span className="tag">{frame ? `CHRONON ${frame.tick}` : 'STANDBY'}</span></div>
    <div className="arena-field">
      <svg viewBox="0 0 300 300" aria-label="Battle arena" role="img">
        <defs><pattern id="grid" width="15" height="15" patternUnits="userSpaceOnUse"><path d="M15 0H0V15" fill="none" stroke="#263438" strokeWidth=".65" /></pattern></defs>
        <rect width="300" height="300" fill="url(#grid)" />
        <circle cx="150" cy="150" r="94" stroke="#263438" fill="none" />
        <path d="M142 150h16m-8-8v16" stroke="#37464a" />
        {frame?.shots.map(([x, y], index) => <circle key={index} cx={x} cy={y} r="1.7" fill="#fff1ae" />)}
        {frame?.bots.map((bot, index) => bot.hp > 0 && <g key={index} transform={`translate(${bot.x} ${bot.y}) rotate(${bot.aim})`}>
          {bot.shield > 0 && <circle r="12" fill="none" stroke={colors[index]} opacity=".45" />}
          <rect x="-6" y="-6" width="12" height="12" rx="2" fill="#182725" stroke={colors[index]} />
          <path d="M0 2V-10" stroke={colors[index]} strokeWidth="2.5" />
        </g>)}
      </svg>
      {!frame && <p className="empty-arena">Waiting for a match</p>}
    </div>
    {frame && <>
      <div className="replay-controls">
        <button onClick={() => {
          if (finished) { setPosition(0); setPlaying(true); }
          else setPlaying(!playing);
        }}>{finished ? 'Replay' : playing ? 'Pause' : 'Play'}</button>
        <input aria-label="Replay position" type="range" min="0" max={frames.length - 1} value={position}
          onChange={event => { setPlaying(false); setPosition(Number(event.target.value)); }} />
        <output>{finished ? replay.winner === undefined ? 'Draw' : `${replay.names[replay.winner]} wins` : 'Playback'}</output>
      </div>
      <div className="telemetry">
        {frame.bots.map((bot, index) => <div key={index}>
          <span style={{ color: colors[index] }}>{replay.names[index]}</span>
          <meter min="0" max="100" value={Math.max(0, bot.hp)} aria-label={`${replay.names[index]} health`} />
          <small>{Math.max(0, bot.hp)} HP · {bot.energy} E</small>
        </div>)}
      </div>
    </>}
  </section>;
}

function App() {
  const connection = useSpacetimeDB();
  const [accounts] = useTable(tables.account);
  const [robots] = useTable(tables.robot);
  const [sources, sourcesReady] = useTable(tables.mySources);
  const [lobbies] = useTable(tables.lobby);
  const [seats] = useTable(tables.seat);
  const [replays] = useTable(tables.replay);
  const [ladder] = useTable(tables.ladder);
  const [page, setPage] = useState('Arena');
  const [handle, setHandle] = useState('');
  const [lobbyName, setLobbyName] = useState('');
  const [edit, setEdit] = useState({ id: undefined, name: '', source: initialSource });
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [pending, setPending] = useState(false);

  const account = accounts.find(row => same(row.identity, connection.identity));
  const myRobots = robots.filter(row => same(row.owner, connection.identity));
  const seat = seats.find(row => same(row.identity, connection.identity));
  const room = lobbies.find(row => row.id === seat?.lobby);
  const members = seats.filter(row => row.lobby === room?.id);
  const isHost = same(room?.host, connection.identity);
  const replay = replays.find(row => row.lobby === room?.id);
  const ranked = [...robots].sort((a, b) => Number(b.rank - a.rank));
  const latest = ladder[0];

  async function run(action, message = '') {
    const reducers = connection.getConnection()?.reducers;
    if (!reducers || pending) return;
    setPending(true);
    setNotice('');
    setError('');
    try {
      await action(reducers);
      setNotice(message);
    } catch (error) {
      setError(error.message || String(error));
    } finally {
      setPending(false);
    }
  }

  function chooseEditor(id) {
    const bot = myRobots.find(row => String(row.id) === id);
    const source = sources.find(row => row.id === bot?.id);
    if (bot && !source) return;
    setEdit(bot ? { id: bot.id, name: bot.name, source: source.source } : { id: undefined, name: '', source: initialSource });
    setNotice('');
  }

  const ownerName = identity => accounts.find(row => same(row.identity, identity))?.handle || 'Unknown';
  return <>
    <header>
      <div className="brand"><span className="brand-mark" aria-hidden="true">⌖</span> RoboWar<span>Net</span></div>
      <nav aria-label="Main navigation">
        {['Arena', 'TV', 'Robot Bay', 'Ladder'].map(tab => <button key={tab} aria-current={page === tab ? 'page' : undefined} onClick={() => setPage(tab)}>{tab}</button>)}
      </nav>
      <span className="connection"><i className={connection.isActive ? 'online' : ''} />{account?.handle || (connection.isActive ? 'Connected' : 'Disconnected')}</span>
    </header>
    <main>
      {!account && <form className="panel account-form" onSubmit={event => {
        event.preventDefault();
        run(reducers => reducers.register({ handle }), 'Account created.');
      }}>
        <div><h2>Choose your handle</h2><p>Create an account to save robots and enter lobbies.</p></div>
        <label className="sr-only" htmlFor="handle">Handle</label>
        <input id="handle" placeholder="your-handle" value={handle} onChange={event => setHandle(event.target.value)} required minLength="3" maxLength="24" />
        <button className="primary" disabled={!connection.isActive || pending}>Create account</button>
      </form>}
      {connection.connectionError && <p className="alert" role="alert">{String(connection.connectionError)}</p>}
      {error && <p className="alert" role="alert">{error}</p>}
      {notice && <p className="notice" role="status">{notice}</p>}
      {page === 'Arena' && <div className="arena-layout">
        <Arena key={String(room?.id)} replay={replay} />
        <section className="panel lobby">
          <div className="panel-heading"><h2>{room ? room.name : 'Lobbies'}</h2>{room && <span className="tag">{members.length}/6</span>}</div>
          {room ? <>
            <ul className="rows">{members.map(member => <li key={member.identity.toHexString()}>
              <span>{ownerName(member.identity)}{same(room.host, member.identity) && <small> · Host</small>}</span>
              <small>{robots.find(robot => robot.id === member.robot)?.name || 'Choosing robot'}</small>
            </li>)}</ul>
            <label htmlFor="entry">Your entry</label>
            <select id="entry" value={seat?.robot?.toString() || ''} disabled={pending || !myRobots.length} onChange={event => {
              const id = BigInt(event.target.value);
              run(reducers => reducers.chooseRobot({ id }));
            }}>
              <option value="" disabled>{myRobots.length ? 'Choose a robot' : 'Create a robot in Robot Bay'}</option>
              {myRobots.map(robot => <option key={String(robot.id)} value={String(robot.id)}>{robot.name}</option>)}
            </select>
            <div className="actions">
              {isHost && <button className="primary" disabled={pending || members.length < 2 || members.some(member => member.robot === undefined)} onClick={() => run(reducers => reducers.startMatch({ id: room.id }))}>Run match</button>}
              <button disabled={pending} onClick={() => run(reducers => reducers.leaveLobby({}))}>Leave lobby</button>
            </div>
          </> : <>
            {lobbies.length ? <ul className="rows">{lobbies.map(lobby => <li key={String(lobby.id)}>
              <span>{lobby.name}<small> · {seats.filter(seat => seat.lobby === lobby.id).length}/6</small></span>
              <button disabled={!account || pending} onClick={() => run(reducers => reducers.joinLobby({ id: lobby.id }))}>Join</button>
            </li>)}</ul> : <p className="muted">No open lobbies.</p>}
            <form className="lobby-form" onSubmit={event => {
              event.preventDefault();
              run(reducers => reducers.createLobby({ name: lobbyName }));
            }}>
              <label htmlFor="lobby-name">New lobby</label>
              <input id="lobby-name" placeholder="Lobby name" value={lobbyName} onChange={event => setLobbyName(event.target.value)} required maxLength="40" />
              <button className="primary" disabled={!account || pending}>Create lobby</button>
            </form>
          </>}
        </section>
      </div>}
      {page === 'TV' && <div className="arena-layout">
        <Arena replay={replays.find(row => row.lobby === 0n)} title="RoboWar TV" />
        <section className="panel">
          <div className="panel-heading"><h2>On air</h2><span className="tag">EXHIBITION</span></div>
          <p className="muted">Random robots. Random match size. A new match every 20 seconds.</p>
          <ul className="rows">{replays.find(row => row.lobby === 0n)?.names.map((name, index) =>
            <li key={index}><span style={{ color: colors[index] }}>{name}</span></li>)}</ul>
        </section>
      </div>}
      {page === 'Robot Bay'  && <section className="panel workshop">
        <div className="panel-heading"><h2>Robot Bay</h2><span className="tag">RoboTalk</span></div>
        <form onSubmit={event => {
          event.preventDefault();
          run(reducers => reducers.saveRobot(edit), 'Robot saved.');
        }}>
          <div className="edit-fields">
            <label>Robot<select value={edit.id?.toString() || ''} onChange={event => chooseEditor(event.target.value)} disabled={!sourcesReady}>
              <option value="">New robot</option>
              {myRobots.map(robot => <option key={String(robot.id)} value={String(robot.id)}>{robot.name}</option>)}
            </select></label>
            <label>Name<input required maxLength="40" value={edit.name} onChange={event => setEdit({ ...edit, name: event.target.value })} /></label>
          </div>
          <label>Source code<textarea value={edit.source} onChange={event => setEdit({ ...edit, source: event.target.value })} required maxLength="32768" spellCheck="false" /></label>
          <button className="primary" disabled={!account || pending}>Save robot</button>
        </form>
      </section>}
      {page === 'Ladder' && <section className="panel leaderboard">
        <div className="panel-heading"><h2>Ladder</h2>{latest && <span className="tag">SWEEP {String(latest.sweep)} · {Math.min(robots.length, 2 + Number((latest.sweep - 1n) % 9n))} BOTS</span>}</div>
        {latest?.lower && <p className="last-bout">{latest.lower}{latest.higher && ` vs. ${latest.higher}`} <span>{latest.winner === 'Draw' ? 'Draw' : `Winner: ${latest.winner}`}</span></p>}
        <table><thead><tr><th>Rank</th><th>Robot</th><th>Owner</th><th>Bouts</th></tr></thead>
          <tbody>{ranked.map(bot => <tr key={String(bot.id)}><td>{String(bot.rank)}</td><td>{bot.name}</td><td>{ownerName(bot.owner)}</td><td>{String(bot.bouts)}</td></tr>)}</tbody>
        </table>
      </section>}
    </main>
  </>;
}

createRoot(document.getElementById('root')).render(
  <SpacetimeDBProvider connectionBuilder={builder}><App /></SpacetimeDBProvider>
);
