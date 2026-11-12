import assert from 'node:assert/strict';
import { DbConnection } from '../bindings';

const uri = Bun.env.VITE_STDB_URL || 'ws://127.0.0.1:3000';
const database = Bun.env.VITE_STDB_DATABASE || 'robowarnet';
const queries = ['account', 'robot', 'my_sources', 'lobby', 'seat', 'replay', 'ladder'].map(name => 'SELECT * FROM ' + name);

async function connect(token?: string) {
  return new Promise<{ connection: DbConnection, token: string }>((resolve, reject) => {
    DbConnection.builder().withUri(uri).withDatabaseName(database).withToken(token)
      .onConnectError((_ctx, error) => reject(error))
      .onConnect((connection, _identity, authToken) => {
        connection.subscriptionBuilder()
          .onError(ctx => reject(ctx.event))
          .onApplied(() => resolve({ connection, token: authToken }))
          .subscribe(queries);
      }).build();
  });
}
async function waitUntil(predicate: () => boolean) {
  const timeout = Date.now() + 6000;
  while (!predicate()) {
    if (Date.now() > timeout) throw new Error('Timed out waiting for subscription data');
    await Bun.sleep(30);
  }
}

const tag = Date.now().toString(36);
const { connection: a, token } = await connect();
const { connection: b } = await connect();
try {
  const initialSweep = [...a.db.ladder.iter()][0].sweep;
  const legacy = [...a.db.account.iter()].find(row => row.handle === 'legacy');
  assert.ok(legacy);
  assert.ok([...a.db.robot.iter()].filter(row => row.owner.isEqual(legacy.identity)).length >= 21);
  await a.reducers.register({ handle: 'alpha-' + tag });
  await b.reducers.register({ handle: 'bravo-' + tag });
  const firstSource = "MAIN: 20 FIRE' STORE SYNC MAIN JUMP";
  const secondSource = 'MAIN: SYNC MAIN JUMP';
  await a.reducers.saveRobot({ id: undefined, name: 'Alpha ' + tag, source: firstSource });
  await b.reducers.saveRobot({ id: undefined, name: 'Bravo ' + tag, source: secondSource });
  await waitUntil(() => a.db.mySources.count() === 1n && b.db.mySources.count() === 1n);
  const [first] = [...a.db.mySources.iter()];
  const [second] = [...b.db.mySources.iter()];
  assert.equal(first.source, firstSource);
  assert.equal(second.source, secondSource);
  assert.notEqual(first.id, second.id);

  const { connection: restored } = await connect(token);
  assert.equal([...restored.db.mySources.iter()][0].source, firstSource);
  restored.disconnect();
  await assert.rejects(b.reducers.saveRobot({ id: first.id, name: 'Takeover', source: secondSource }));

  await a.reducers.createLobby({ name: 'Smoke ' + tag });
  const [room] = [...a.db.lobby.iter()].filter(row => row.name === 'Smoke ' + tag);
  assert.ok(room);
  await b.reducers.joinLobby({ id: room.id });
  await assert.rejects(b.reducers.chooseRobot({ id: first.id }));
  await a.reducers.chooseRobot({ id: first.id });
  await b.reducers.chooseRobot({ id: second.id });
  await assert.rejects(b.reducers.startMatch({ id: room.id }));
  await a.reducers.startMatch({ id: room.id });
  await waitUntil(() => [...b.db.replay.iter()].some(row => row.lobby === room.id));
  const replay = [...b.db.replay.iter()].find(row => row.lobby === room.id)!;
  assert.equal(replay.names[replay.winner!], 'Alpha ' + tag);
  assert.ok(JSON.parse(replay.frames).length > 1);
  assert.ok(!replay.frames.includes(firstSource));
  await a.reducers.leaveLobby({});
  await waitUntil(() => [...b.db.lobby.iter()].some(row => row.id === room.id && row.host.isEqual(b.identity!)));
  await b.reducers.leaveLobby({});
  await waitUntil(() => [...a.db.ladder.iter()][0].sweep > initialSweep);
  const ranks = [...a.db.robot.iter()].map(row => Number(row.rank)).sort((left, right) => left - right);
  assert.deepEqual(ranks, ranks.map((_, index) => index));
  console.log('Verified identity restore, private source, ownership, lobby, server replay, and ladder.');
} finally {
  a.disconnect();
  b.disconnect();
}
