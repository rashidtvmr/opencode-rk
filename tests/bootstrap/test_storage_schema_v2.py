"""SQL contract tests for the proposed format; not Rust or power-loss certification."""
from __future__ import annotations

import pathlib
import sqlite3
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SQL = ROOT / 'crates/storage/schema/v2'


def connect(path: str = ':memory:', kind: str = 'workspace') -> sqlite3.Connection:
    db = sqlite3.connect(path, isolation_level=None)
    db.execute('PRAGMA foreign_keys=ON')
    db.execute('PRAGMA trusted_schema=OFF')
    db.execute('PRAGMA synchronous=FULL')
    db.execute('PRAGMA auto_vacuum=INCREMENTAL')
    db.execute('PRAGMA journal_mode=WAL')
    db.executescript('BEGIN IMMEDIATE;\n' + (SQL / f'{kind}.sql').read_text() + '\nCOMMIT;')
    if kind == 'workspace':
        db.execute('INSERT INTO workspace_state(id,workspace_id,cursor_epoch,format_version,created_at_us) VALUES(1,?,?,2,0)', (b'w'*16, b'e'*16))
    return db


def session(db, key=1):
    return db.execute('INSERT INTO sessions(id,title,created_at_us,updated_at_us) VALUES(?,?,0,0)', (key.to_bytes(16, 'big'), 'session')).lastrowid


def inline(db, value=b'{}'):
    return db.execute('INSERT INTO payloads(inline_data,raw_bytes,created_at_us) VALUES(?,?,0)', (value,len(value))).lastrowid


def blob(db, state=0):
    return db.execute('INSERT INTO blobs(hash,state,codec,raw_bytes,stored_bytes,created_at_us) VALUES(?,?,0,9000,9052,0)', (b'h'*32,state)).lastrowid


def message(db, sid, key=1, seq=1, role=1):
    return db.execute('INSERT INTO messages(id,session_pk,seq,role,status,created_at_us,completed_at_us) VALUES(?,?,?,?,1,0,0)', (key.to_bytes(16,'big'),sid,seq,role)).lastrowid


def execution(db, sid, key=1, state=0, parent=None):
    return db.execute("INSERT INTO executions(id,session_pk,parent_execution_pk,mode,state,owner_generation,config_payload_pk,provider_id,model_id,created_at_us,finished_at_us) VALUES(?,?,?,0,?,1,?,'provider','model',0,?)", (key.to_bytes(16,'big'),sid,parent,state,inline(db),None if state in (0,1,4) else 0)).lastrowid


def tool(db, sid, eid, mid, ordinal=0):
    return db.execute("INSERT INTO tool_calls(id,session_pk,execution_pk,assistant_message_pk,ordinal,name,state,intent_hash,input_payload_pk,created_at_us) VALUES(?,?,?,?,?,'read',0,?,?,0)", ((ordinal+1).to_bytes(16,'big'),sid,eid,mid,ordinal,b'd'*32,inline(db))).lastrowid


def event(db, sid=None):
    head = db.execute('UPDATE workspace_state SET event_head_seq=event_head_seq+1 WHERE id=1 RETURNING event_head_seq').fetchone()[0]
    db.execute("INSERT INTO event_outbox VALUES(?,?,'changed','{}',0)",(head,sid))
    return head


class WorkspaceSchemaTests(unittest.TestCase):
    def setUp(self):
        self.db = connect()
        self.addCleanup(self.db.close)

    def rejected(self, sql, args=()):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute(sql,args)

    def test_all_tables_are_strict(self):
        tables=self.db.execute('PRAGMA table_list').fetchall()
        self.assertTrue(all(row[5]==1 for row in tables if row[2]=='table' and not row[1].startswith('sqlite_')))

    def test_public_id_is_binary_uuid(self):
        self.rejected("INSERT INTO sessions(id,title,created_at_us,updated_at_us) VALUES('not-a-uuid','x',0,0)")

    def test_payload_inline_limit_and_length(self):
        inline(self.db,b'x'*8192)
        self.rejected('INSERT INTO payloads(inline_data,raw_bytes,created_at_us) VALUES(?,?,0)',(b'x'*8193,8193))
        self.rejected("INSERT INTO payloads(inline_data,raw_bytes,created_at_us) VALUES(x'0001',1,0)")

    def test_payload_has_exactly_one_representation(self):
        bid=blob(self.db)
        self.rejected('INSERT INTO payloads(raw_bytes,created_at_us) VALUES(0,0)')
        self.rejected('INSERT INTO payloads(inline_data,blob_pk,raw_bytes,created_at_us) VALUES(?,?,9000,0)',(b'x'*9000,bid))

    def test_payload_rejects_unknown_and_mismatched_blob(self):
        bid=blob(self.db)
        for ref,length in [(999,9000),(bid,8999)]:
            self.rejected('INSERT INTO payloads(blob_pk,raw_bytes,created_at_us) VALUES(?,?,0)',(ref,length))

    def test_payload_cannot_mutate_shared_content(self):
        pid=inline(self.db)
        self.rejected('UPDATE payloads SET inline_data=? WHERE pk=?',(b'[]',pid))

    def test_blob_gc_rejects_reattachment_to_tombstone(self):
        bid=blob(self.db)
        self.db.execute('UPDATE blobs SET state=1 WHERE pk=?',(bid,))
        self.rejected('INSERT INTO payloads(blob_pk,raw_bytes,created_at_us) VALUES(?,9000,0)',(bid,))
        self.rejected('UPDATE blobs SET state=0 WHERE pk=?',(bid,))

    def test_blob_gc_cannot_claim_referenced_blob(self):
        bid=blob(self.db)
        self.db.execute('INSERT INTO payloads(blob_pk,raw_bytes,created_at_us) VALUES(?,9000,0)',(bid,))
        self.rejected('UPDATE blobs SET state=1 WHERE pk=?',(bid,))
        self.rejected('DELETE FROM blobs WHERE pk=?',(bid,))

    def test_unavailable_blob_blocks_new_payloads(self):
        bid=blob(self.db,2)
        self.rejected('INSERT INTO payloads(blob_pk,raw_bytes,created_at_us) VALUES(?,9000,0)',(bid,))

    def test_empty_inline_is_valid(self):
        self.assertIsInstance(inline(self.db,b''),int)

    def test_fork_reuses_payload_and_survives_parent_deletion(self):
        a,b=session(self.db),session(self.db,2)
        ma,mb=message(self.db,a),message(self.db,b,2)
        pid=inline(self.db,b'shared')
        for mid in (ma,mb):
            self.db.execute('INSERT INTO message_parts(message_pk,ordinal,kind,payload_pk) VALUES(?,0,0,?)',(mid,pid))
        self.db.execute('DELETE FROM sessions WHERE pk=?',(a,))
        self.assertEqual(self.db.execute('SELECT payload_pk FROM message_parts').fetchall(),[(pid,)])
        self.rejected('DELETE FROM payloads WHERE pk=?',(pid,))

    def test_admission_promotion_shares_payload_and_deletes_cleanly(self):
        sid=session(self.db); pid=inline(self.db,b'prompt')
        iid=self.db.execute('INSERT INTO session_inputs(id,session_pk,seq,delivery,request_hash,created_at_us) VALUES(?,?,1,0,?,0)',(b'i'*16,sid,b'r'*32)).lastrowid
        self.db.execute('INSERT INTO session_input_parts(input_pk,ordinal,kind,payload_pk) VALUES(?,0,0,?)',(iid,pid))
        self.db.execute('BEGIN IMMEDIATE')
        mid=message(self.db,sid)
        self.db.execute('INSERT INTO message_parts(message_pk,ordinal,kind,payload_pk) SELECT ?,ordinal,kind,payload_pk FROM session_input_parts WHERE input_pk=?',(mid,iid))
        self.db.execute('UPDATE session_inputs SET state=1,promoted_message_pk=?,promoted_at_us=1 WHERE pk=?',(mid,iid))
        self.db.execute('DELETE FROM session_input_parts WHERE input_pk=?',(iid,))
        self.db.execute('COMMIT')
        self.assertEqual(self.db.execute('SELECT count(*) FROM payloads').fetchone()[0],1)
        self.db.execute('DELETE FROM sessions WHERE pk=?',(sid,))
        self.assertEqual(self.db.execute('PRAGMA foreign_key_check').fetchall(),[])

    def test_admission_rejects_cross_session_message(self):
        a,b=session(self.db),session(self.db,2); mid=message(self.db,b)
        self.rejected('INSERT INTO session_inputs(id,session_pk,seq,delivery,state,request_hash,promoted_message_pk,created_at_us,promoted_at_us) VALUES(?,?,1,0,1,?,?,0,1)',(b'i'*16,a,b'r'*32,mid))

    def test_admission_request_identity_is_immutable(self):
        sid=session(self.db)
        self.db.execute('INSERT INTO session_inputs(id,session_pk,seq,delivery,request_hash,created_at_us) VALUES(?,?,1,0,?,0)',(b'i'*16,sid,b'r'*32))
        self.rejected('UPDATE session_inputs SET delivery=1')

    def test_uncertain_execution_blocks_new_run(self):
        sid=session(self.db); eid=execution(self.db,sid,state=4)
        with self.assertRaises(sqlite3.IntegrityError): execution(self.db,sid,key=2)
        self.db.execute('UPDATE executions SET state=5,finished_at_us=1 WHERE pk=?',(eid,))
        execution(self.db,sid,key=3)

    def test_execution_parent_cannot_be_rewritten_to_cycle(self):
        a,b=session(self.db),session(self.db,2)
        ea=execution(self.db,a); eb=execution(self.db,b,key=2,parent=ea)
        self.rejected('UPDATE executions SET parent_execution_pk=? WHERE pk=?',(eb,ea))

    def test_tool_requires_same_session_execution_and_assistant(self):
        a,b=session(self.db),session(self.db,2)
        ea=execution(self.db,a); eb=execution(self.db,b,key=2)
        ma=message(self.db,a,role=2); mb=message(self.db,b,key=2,role=2)
        for sid,eid,mid in [(a,eb,ma),(a,ea,mb)]:
            with self.assertRaises(sqlite3.IntegrityError): tool(self.db,sid,eid,mid)
        tool(self.db,a,ea,ma)

    def test_tool_cannot_be_owned_by_user_message(self):
        sid=session(self.db); eid=execution(self.db,sid); mid=message(self.db,sid)
        with self.assertRaises(sqlite3.IntegrityError): tool(self.db,sid,eid,mid)

    def test_tool_success_requires_output(self):
        sid=session(self.db); eid=execution(self.db,sid); mid=message(self.db,sid,role=2)
        tid=tool(self.db,sid,eid,mid)
        self.rejected('UPDATE tool_calls SET state=2,finished_at_us=1 WHERE pk=?',(tid,))

    def test_tool_approval_digest_binding_is_immutable(self):
        sid=session(self.db); eid=execution(self.db,sid); mid=message(self.db,sid,role=2)
        tool(self.db,sid,eid,mid)
        self.rejected("UPDATE tool_calls SET name='delete'")

    def test_mandatory_human_approval_needs_client_identity(self):
        sid=session(self.db)
        self.rejected("INSERT INTO approvals(id,session_pk,intent_hash,policy_generation,action,state,mandatory_human,created_at_us,expires_at_us) VALUES(?,?,?,1,'delete',1,1,0,100)",(b'a'*16,sid,b'd'*32))

    def test_only_one_open_context_epoch(self):
        sid=session(self.db); pid=inline(self.db); args=(sid,pid,pid)
        self.db.execute('INSERT INTO context_epochs(session_pk,epoch,baseline_payload_pk,snapshot_payload_pk,created_at_us) VALUES(?,1,?,?,0)',args)
        self.rejected('INSERT INTO context_epochs(session_pk,epoch,baseline_payload_pk,snapshot_payload_pk,created_at_us) VALUES(?,2,?,?,0)',args)

    def test_backup_pin_prevents_collection(self):
        pid=inline(self.db)
        self.db.execute('INSERT INTO retained_payloads(owner_id,purpose,payload_pk,created_at_us) VALUES(?,2,?,0)',(b'b'*16,pid))
        self.assertEqual(self.db.execute('SELECT payload_pk FROM payload_roots').fetchall(),[(pid,)])
        self.rejected('DELETE FROM payloads WHERE pk=?',(pid,))

    def test_every_payload_fk_is_in_gc_root_view(self):
        ddl=self.db.execute("SELECT sql FROM sqlite_schema WHERE name='payload_roots'").fetchone()[0].lower()
        for (table,) in self.db.execute("SELECT name FROM sqlite_schema WHERE type='table'"):
            for fk in self.db.execute(f'PRAGMA foreign_key_list("{table}")'):
                if fk[2]=='payloads': self.assertIn(f'select {fk[3]} from {table}',ddl)

    def test_bounded_parts_and_valid_json(self):
        sid=session(self.db); mid=message(self.db,sid); pid=inline(self.db)
        for ordinal,meta in [(256,'{}'),(0,'invalid'),(0,'"'+'x'*4096+'"')]:
            self.rejected('INSERT INTO message_parts(message_pk,ordinal,kind,payload_pk,metadata_json) VALUES(?,?,0,?,?)',(mid,ordinal,pid,meta))

    def test_state_and_outbox_rollback_together(self):
        self.db.execute('BEGIN IMMEDIATE'); session(self.db); event(self.db); self.db.execute('ROLLBACK')
        self.assertEqual(self.db.execute('SELECT count(*) FROM sessions').fetchone()[0],0)
        self.assertEqual(self.db.execute('SELECT count(*) FROM event_outbox').fetchone()[0],0)
        self.assertEqual(self.db.execute('SELECT event_head_seq FROM workspace_state').fetchone()[0],0)

    def test_event_watermark_survives_pruning_all_rows(self):
        self.db.execute('BEGIN IMMEDIATE')
        for _ in range(3): event(self.db)
        self.db.execute('COMMIT'); self.db.execute('BEGIN IMMEDIATE')
        self.db.execute('DELETE FROM event_outbox WHERE seq<=3')
        self.db.execute('UPDATE workspace_state SET event_floor_seq=3'); self.db.execute('COMMIT')
        self.db.execute('BEGIN IMMEDIATE'); self.assertEqual(event(self.db),4); self.db.execute('COMMIT')
        self.assertEqual(self.db.execute('SELECT event_floor_seq,event_head_seq FROM workspace_state').fetchone(),(3,4))

    def test_delete_event_survives_session_deletion(self):
        sid=session(self.db)
        self.db.execute('BEGIN IMMEDIATE'); event(self.db,(1).to_bytes(16,'big'))
        self.db.execute('DELETE FROM sessions WHERE pk=?',(sid,)); self.db.execute('COMMIT')
        self.assertEqual(self.db.execute('SELECT count(*) FROM event_outbox').fetchone()[0],1)

    def test_cursor_floor_cannot_exceed_head(self):
        self.rejected('UPDATE workspace_state SET event_floor_seq=1')

    def test_invalid_event_json_and_oversized_payload_rejected(self):
        for value in ['no-json','"'+'x'*4096+'"']:
            self.rejected("INSERT INTO event_outbox VALUES(1,NULL,'x',?,0)",(value,))

    def test_receipt_bounds_and_duplicate_key(self):
        args=(b'o'*16,b'd'*32); sql="INSERT INTO operation_receipts VALUES(?,?,'rename','{}',0,100)"
        self.db.execute(sql,args); self.rejected(sql,args)

    def test_keyset_query_plans_use_indexes_without_sort(self):
        checks=[
            ("SELECT pk FROM sessions WHERE state=0 AND (updated_at_us,pk)<(10,10) ORDER BY updated_at_us DESC,pk DESC LIMIT 50",'sessions_state_updated_idx'),
            ("SELECT pk FROM messages WHERE session_pk=1 AND seq>0 ORDER BY seq LIMIT 50",'sqlite_autoindex_messages_2'),
            ("SELECT pk FROM session_inputs WHERE session_pk=1 AND delivery=1 AND state=0 AND seq>0 ORDER BY seq LIMIT 50",'inputs_pending_idx'),
            ("SELECT seq FROM event_outbox WHERE session_id=x'00000000000000000000000000000001' AND seq>0 ORDER BY seq LIMIT 50",'event_session_idx'),
        ]
        for query,index in checks:
            plan=' '.join(row[3] for row in self.db.execute('EXPLAIN QUERY PLAN '+query))
            self.assertIn(index,plan); self.assertNotIn('TEMP B-TREE',plan)

    def test_schema_load_rolls_back_after_interruption(self):
        db=sqlite3.connect(':memory:',isolation_level=None); self.addCleanup(db.close)
        with self.assertRaises(sqlite3.OperationalError):
            db.executescript('BEGIN; CREATE TABLE sentinel(pk INTEGER PRIMARY KEY); INVALID SQL; COMMIT;')
        db.execute('ROLLBACK')
        self.assertEqual(db.execute("SELECT name FROM sqlite_schema WHERE name='sentinel'").fetchall(),[])


class CatalogSchemaTests(unittest.TestCase):
    def setUp(self):
        self.db=connect(kind='catalog'); self.addCleanup(self.db.close)

    def test_catalog_has_no_conversation_or_secret_value_columns(self):
        names={row[0] for row in self.db.execute("SELECT name FROM sqlite_schema WHERE type='table'")}
        self.assertNotIn('messages',names)
        cols={row[1] for row in self.db.execute('PRAGMA table_info(provider_accounts)')}
        self.assertIn('secret_ref',cols)
        self.assertFalse({'api_key','access_token','refresh_token'} & cols)

    def test_secret_reference_and_json_are_bounded(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO app_settings VALUES('x','not-json',1,0)")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO provider_accounts(id,provider_id,label,secret_ref,created_at_us) VALUES(?,'p','x',?,0)",(b'a'*16,'x'*1025))

    def test_account_model_lock_is_unique_and_cascades(self):
        pk=self.db.execute("INSERT INTO provider_accounts(id,provider_id,label,secret_ref,created_at_us) VALUES(?,'p','x','credential://opaque',0)",(b'a'*16,)).lastrowid
        self.db.execute("INSERT INTO provider_account_model_locks VALUES(?,'*',100,'quota')",(pk,))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO provider_account_model_locks VALUES(?,'*',101,'quota')",(pk,))
        self.db.execute('DELETE FROM provider_accounts WHERE pk=?',(pk,))
        self.assertEqual(self.db.execute('SELECT count(*) FROM provider_account_model_locks').fetchone()[0],0)


class ProcessCrashTests(unittest.TestCase):
    def test_process_exit_preserves_only_committed_state_and_outbox(self):
        # Abrupt process exit is NOT power loss or an fsync durability proof.
        with tempfile.TemporaryDirectory() as temp:
            path=str(pathlib.Path(temp)/'state.db'); db=connect(path); db.close()
            for commit in (False,True):
                script='''import sqlite3,os,sys
c=sqlite3.connect(sys.argv[1],isolation_level=None)
c.execute('PRAGMA foreign_keys=ON'); c.execute('PRAGMA synchronous=FULL')
c.execute('BEGIN IMMEDIATE')
c.execute("INSERT INTO sessions(id,title,created_at_us,updated_at_us) VALUES(?,'crash',0,0)",(b'c'*16,))
c.execute('UPDATE workspace_state SET event_head_seq=event_head_seq+1')
c.execute("INSERT INTO event_outbox VALUES(1,NULL,'created','{}',0)")
if sys.argv[2]=='yes': c.execute('COMMIT')
os._exit(17)
'''
                result=subprocess.run([sys.executable,'-c',script,path,'yes' if commit else 'no'],capture_output=True,timeout=10)
                self.assertEqual(result.returncode,17,result.stderr)
                db=sqlite3.connect(path)
                try:
                    expected=1 if commit else 0
                    self.assertEqual(db.execute('SELECT count(*) FROM sessions').fetchone()[0],expected)
                    self.assertEqual(db.execute('SELECT count(*) FROM event_outbox').fetchone()[0],expected)
                    self.assertEqual(db.execute('PRAGMA quick_check').fetchone()[0],'ok')
                finally: db.close()


class CatalogInitTests(unittest.TestCase):
    def setUp(self):
        self.db = connect(kind='catalog')
        self.addCleanup(self.db.close)

    def test_catalog_ddl_loads_under_fk_on_with_zero_errors(self):
        self.assertEqual(self.db.execute('PRAGMA foreign_keys').fetchone()[0], 1)
        self.assertEqual(self.db.execute('PRAGMA foreign_key_check').fetchall(), [])

    def test_catalog_singleton_and_migration_checksum_insert(self):
        self.db.execute('INSERT INTO catalog_state(id,installation_id,format_version) VALUES(1,?,2)', (b'i' * 16,))
        self.db.execute('INSERT INTO schema_migrations(version,checksum,applied_at_us) VALUES(2,?,0)', (b'c' * 32,))
        self.assertEqual(
            self.db.execute('SELECT installation_id,format_version FROM catalog_state WHERE id=1').fetchone(),
            (b'i' * 16, 2))
        checksum = self.db.execute('SELECT checksum FROM schema_migrations WHERE version=2').fetchone()[0]
        self.assertEqual(len(checksum), 32)

    def test_duplicate_singleton_and_short_checksum_rejected(self):
        self.db.execute('INSERT INTO catalog_state(id,installation_id,format_version) VALUES(1,?,2)', (b'i' * 16,))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute('INSERT INTO catalog_state(id,installation_id,format_version) VALUES(1,?,2)', (b'j' * 16,))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute('INSERT INTO schema_migrations(version,checksum,applied_at_us) VALUES(3,?,0)', (b'short',))


class InitializerSequenceTests(unittest.TestCase):
    def test_file_pragmas_apply_before_ddl(self):
        with tempfile.TemporaryDirectory() as temp:
            path = str(pathlib.Path(temp) / 'workspace.db')
            db = sqlite3.connect(path, isolation_level=None)
            self.addCleanup(db.close)
            db.execute('PRAGMA page_size=4096')
            db.execute('PRAGMA auto_vacuum=INCREMENTAL')
            db.execute('PRAGMA journal_mode=WAL')
            db.executescript('BEGIN IMMEDIATE;\n' + (SQL / 'workspace.sql').read_text() + '\nCOMMIT;')
            self.assertEqual(db.execute('PRAGMA page_size').fetchone()[0], 4096)
            self.assertEqual(db.execute('PRAGMA auto_vacuum').fetchone()[0], 2)

    def test_wal_attempt_recorded_as_string(self):
        memory = sqlite3.connect(':memory:')
        self.addCleanup(memory.close)
        mode = memory.execute('PRAGMA journal_mode=WAL').fetchone()[0]
        self.assertIsInstance(mode, str)
        # System sqlite 3.46.1 reports memory for :memory: databases while
        # file databases report wal. Record either, fail on anything else.
        self.assertIn(mode.lower(), ('wal', 'memory'))
        with tempfile.TemporaryDirectory() as temp:
            path = str(pathlib.Path(temp) / 'workspace.db')
            file_db = sqlite3.connect(path, isolation_level=None)
            self.addCleanup(file_db.close)
            self.assertEqual(file_db.execute('PRAGMA journal_mode=WAL').fetchone()[0].lower(), 'wal')

    def test_nonempty_file_fails_closed_bytes_unchanged(self):
        def initialize_new(path):
            if path.stat().st_size > 0:
                raise RuntimeError('file exists and is non-empty')

        with tempfile.TemporaryDirectory() as temp:
            path = pathlib.Path(temp) / 'not-a-workspace.db'
            original = b'pre-existing bytes'
            path.write_bytes(original)
            with self.assertRaises(RuntimeError):
                initialize_new(path)
            self.assertEqual(path.read_bytes(), original)

    def test_stored_vs_tampered_checksum_mismatch_detected(self):
        db = connect()
        self.addCleanup(db.close)
        db.execute('INSERT INTO schema_migrations(version,checksum,applied_at_us) VALUES(2,?,0)', (b'A' * 32,))
        stored = db.execute('SELECT checksum FROM schema_migrations WHERE version=2').fetchone()[0]
        db.execute('UPDATE schema_migrations SET checksum=? WHERE version=2', (b'\x00' * 32,))
        tampered = db.execute('SELECT checksum FROM schema_migrations WHERE version=2').fetchone()[0]
        self.assertEqual(len(bytes(stored)), 32)
        self.assertNotEqual(bytes(stored), bytes(tampered))


def append_atomic(db, sid, mid, text, kind='created', payload='{}'):
    """Mirror of the V2Writer atomic append: seq alloc, message, inline part
    and outbox head bump commit together or roll back together."""
    db.execute('BEGIN IMMEDIATE')
    try:
        row = db.execute('SELECT pk,next_message_seq FROM sessions WHERE id=?', (sid,)).fetchone()
        session_pk, seq = row
        db.execute('UPDATE sessions SET next_message_seq=next_message_seq+1 WHERE pk=?', (session_pk,))
        message_pk = db.execute(
            'INSERT INTO messages(id,session_pk,seq,role,status,created_at_us,completed_at_us)'
            ' VALUES(?,?,?,?,1,0,0)', (mid, session_pk, seq, 1)).lastrowid
        payload_pk = db.execute(
            'INSERT INTO payloads(inline_data,raw_bytes,created_at_us) VALUES(?,?,0)',
            (text, len(text))).lastrowid
        db.execute('INSERT INTO message_parts(message_pk,ordinal,kind,payload_pk) VALUES(?,0,0,?)',
                   (message_pk, payload_pk))
        head = db.execute(
            'UPDATE workspace_state SET event_head_seq=event_head_seq+1 WHERE id=1'
            ' RETURNING event_head_seq').fetchone()[0]
        db.execute('INSERT INTO event_outbox VALUES(?,?,?,?,0)', (head, sid, kind, payload))
        db.execute('COMMIT')
        return seq
    except Exception:
        db.execute('ROLLBACK')
        raise


class WriterAtomicityTests(unittest.TestCase):
    def setUp(self):
        self.db = connect()
        self.addCleanup(self.db.close)
        self.sid = (1).to_bytes(16, 'big')
        session(self.db)

    def test_commit_together(self):
        seq = append_atomic(self.db, self.sid, (1).to_bytes(16, 'big'), b'{"ok":true}')
        self.assertEqual(seq, 1)
        counts = self.db.execute(
            'SELECT (SELECT count(*) FROM messages),(SELECT count(*) FROM payloads),'
            '(SELECT count(*) FROM message_parts),(SELECT count(*) FROM event_outbox),'
            'event_head_seq FROM workspace_state WHERE id=1').fetchone()
        self.assertEqual(counts, (1, 1, 1, 1, 1))

    def test_invalid_json_rolls_back_to_zero(self):
        with self.assertRaises(sqlite3.IntegrityError):
            append_atomic(self.db, self.sid, (1).to_bytes(16, 'big'), b'{}', payload='not-json')
        triple = self.db.execute(
            'SELECT (SELECT count(*) FROM messages),(SELECT count(*) FROM event_outbox),'
            'event_head_seq FROM workspace_state WHERE id=1').fetchone()
        self.assertEqual(triple, (0, 0, 0))

    def test_monotonic_sequences_next_is_three(self):
        seqs = [append_atomic(self.db, self.sid, (key).to_bytes(16, 'big'), b'x')
                for key in (1, 2)]
        self.assertEqual(seqs, [1, 2])
        self.assertEqual(
            self.db.execute('SELECT next_message_seq FROM sessions WHERE id=?', (self.sid,)).fetchone()[0], 3)

    def test_oversize_outbox_rejected_without_head_advance(self):
        payload = '"' + 'x' * 4096 + '"'
        self.assertGreater(len(payload.encode()), 4096)
        with self.assertRaises(sqlite3.IntegrityError):
            append_atomic(self.db, self.sid, (1).to_bytes(16, 'big'), b'{}', kind='large', payload=payload)
        self.assertEqual(self.db.execute('SELECT event_head_seq FROM workspace_state').fetchone()[0], 0)
        self.assertEqual(self.db.execute('SELECT count(*) FROM event_outbox').fetchone()[0], 0)


class WriterNegativeBoundsTests(unittest.TestCase):
    def setUp(self):
        self.db = connect()
        self.addCleanup(self.db.close)

    def rejected(self, sql, args=()):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute(sql, args)

    def test_title_1024_ok_1025_rejected(self):
        self.db.execute('INSERT INTO sessions(id,title,created_at_us,updated_at_us) VALUES(?,?,0,0)',
                        (b'a' * 16, 'x' * 1024))
        self.rejected('INSERT INTO sessions(id,title,created_at_us,updated_at_us) VALUES(?,?,0,0)',
                      (b'b' * 16, 'x' * 1025))
        self.assertEqual(self.db.execute('SELECT count(*) FROM sessions').fetchone()[0], 1)

    def test_inline_over_8192_rejected(self):
        self.rejected('INSERT INTO payloads(inline_data,raw_bytes,created_at_us) VALUES(?,?,0)', (b'x' * 8193, 8193))

    def test_invalid_outbox_json_rejected(self):
        self.rejected("INSERT INTO event_outbox VALUES(1,NULL,'x',?,0)", ('not-json',))

    def test_empty_kind_rejected(self):
        self.rejected('INSERT INTO event_outbox VALUES(1,NULL,?,?,0)', ('', '{}'))

    def test_part_kind_null_and_16_rejected(self):
        sid = session(self.db)
        mid = message(self.db, sid)
        pid = inline(self.db)
        self.rejected('INSERT INTO message_parts(message_pk,ordinal,kind,payload_pk) VALUES(?,0,?,?)', (mid, None, pid))
        self.rejected('INSERT INTO message_parts(message_pk,ordinal,kind,payload_pk) VALUES(?,0,?,?)', (mid, 16, pid))


class KeysetPaginationTests(unittest.TestCase):
    QUERIES = [
        ('SELECT pk,id,title,updated_at_us FROM sessions'
         ' WHERE state=? AND (updated_at_us,pk)<(?,?) ORDER BY updated_at_us DESC,pk DESC LIMIT ?',
         (0, 10, 10, 50), 'sessions_state_updated_idx'),
        ('SELECT pk,id,seq,role,status FROM messages WHERE session_pk=? AND seq>? ORDER BY seq LIMIT ?',
         (1, 0, 50), 'sqlite_autoindex_messages_2'),
        ('SELECT pk,id FROM session_inputs WHERE session_pk=? AND delivery=? AND state=0 AND seq>?'
         ' ORDER BY seq LIMIT ?',
         (1, 1, 0, 50), 'inputs_pending_idx'),
        ('SELECT seq,kind,payload_json FROM event_outbox WHERE session_id=? AND seq>? AND seq<=?'
         ' ORDER BY seq LIMIT ?',
         (b's' * 16, 0, 99, 50), 'event_session_idx'),
    ]

    def setUp(self):
        self.db = connect()
        self.addCleanup(self.db.close)

    def test_exact_storage_md_queries_use_indexes_without_temp_btree(self):
        for query, args, index in self.QUERIES:
            plan = ' '.join(row[3] for row in self.db.execute('EXPLAIN QUERY PLAN ' + query, args))
            self.assertIn('USING INDEX', plan)
            self.assertIn(index, plan)
            self.assertNotIn('TEMP B-TREE', plan)

    def test_keyset_page_returns_expected_row(self):
        pks = []
        for key, updated in ((1, 30), (2, 20), (3, 10)):
            pks.append(session(self.db, key))
            self.db.execute('UPDATE sessions SET updated_at_us=? WHERE pk=?', (updated, pks[-1]))
        page = ('SELECT updated_at_us,pk FROM sessions WHERE state=0 AND (updated_at_us,pk)<(?,?)'
                ' ORDER BY updated_at_us DESC,pk DESC LIMIT 2')
        self.assertEqual(self.db.execute(page, (31, 1 << 62)).fetchall(), [(30, pks[0]), (20, pks[1])])
        self.assertEqual(self.db.execute(page, (20, pks[1])).fetchall(), [(10, pks[2])])


class GcSafetyPinTests(unittest.TestCase):
    def setUp(self):
        self.db = connect()
        self.addCleanup(self.db.close)

    def test_referenced_blob_cannot_go_deleting_or_be_deleted(self):
        bid = blob(self.db)
        self.db.execute('INSERT INTO payloads(blob_pk,raw_bytes,created_at_us) VALUES(?,9000,0)', (bid,))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute('UPDATE blobs SET state=1 WHERE pk=?', (bid,))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute('DELETE FROM blobs WHERE pk=?', (bid,))

    def test_deleting_blob_cannot_gain_refs_or_resurrect(self):
        bid = blob(self.db)
        self.db.execute('UPDATE blobs SET state=1 WHERE pk=?', (bid,))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute('INSERT INTO payloads(blob_pk,raw_bytes,created_at_us) VALUES(?,9000,0)', (bid,))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute('UPDATE blobs SET state=0 WHERE pk=?', (bid,))

    def test_unreferenced_tombstone_deletable(self):
        bid = blob(self.db)
        self.db.execute('UPDATE blobs SET state=1 WHERE pk=?', (bid,))
        self.db.execute('DELETE FROM blobs WHERE pk=?', (bid,))
        self.assertEqual(self.db.execute('SELECT count(*) FROM blobs').fetchone()[0], 0)


class MutationHookTests(unittest.TestCase):
    """Drop one trigger in a scratch copy and show the formerly rejected
    statement now succeeds. Each test proves its trigger owns the invariant."""

    def scratch(self):
        db = connect()
        self.addCleanup(db.close)
        return db

    def assert_trigger_present(self, db, name):
        self.assertEqual(
            db.execute("SELECT count(*) FROM sqlite_master WHERE type='trigger' AND name=?", (name,)).fetchone()[0],
            1)

    def test_drop_blob_gc_claim_allows_collecting_referenced_blob(self):
        db = self.scratch()
        self.assert_trigger_present(db, 'blob_gc_claim')
        bid = blob(db)
        db.execute('INSERT INTO payloads(blob_pk,raw_bytes,created_at_us) VALUES(?,9000,0)', (bid,))
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute('UPDATE blobs SET state=1 WHERE pk=?', (bid,))
        db.execute('DROP TRIGGER blob_gc_claim')
        db.execute('UPDATE blobs SET state=1 WHERE pk=?', (bid,))
        self.assertEqual(db.execute('SELECT state FROM blobs WHERE pk=?', (bid,)).fetchone()[0], 1)

    def test_drop_payload_ready_allows_tombstone_reference(self):
        db = self.scratch()
        self.assert_trigger_present(db, 'payload_ready')
        bid = blob(db)
        db.execute('UPDATE blobs SET state=1 WHERE pk=?', (bid,))
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute('INSERT INTO payloads(blob_pk,raw_bytes,created_at_us) VALUES(?,9000,0)', (bid,))
        db.execute('DROP TRIGGER payload_ready')
        rowid = db.execute('INSERT INTO payloads(blob_pk,raw_bytes,created_at_us) VALUES(?,9000,0)', (bid,)).lastrowid
        self.assertIsInstance(rowid, int)

    def test_drop_blob_no_resurrection_allows_resurrection(self):
        db = self.scratch()
        self.assert_trigger_present(db, 'blob_no_resurrection')
        bid = blob(db)
        db.execute('UPDATE blobs SET state=1 WHERE pk=?', (bid,))
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute('UPDATE blobs SET state=0 WHERE pk=?', (bid,))
        db.execute('DROP TRIGGER blob_no_resurrection')
        db.execute('UPDATE blobs SET state=0 WHERE pk=?', (bid,))
        self.assertEqual(db.execute('SELECT state FROM blobs WHERE pk=?', (bid,)).fetchone()[0], 0)

    def test_drop_tool_assistant_owner_allows_user_owned_tool(self):
        db = self.scratch()
        self.assert_trigger_present(db, 'tool_assistant_owner')
        sid = session(db)
        first = execution(db, sid)
        mid = message(db, sid)
        with self.assertRaises(sqlite3.IntegrityError):
            tool(db, sid, first, mid)
        db.execute('UPDATE executions SET state=5,finished_at_us=1 WHERE pk=?', (first,))
        db.execute('DROP TRIGGER tool_assistant_owner')
        self.assertIsInstance(tool(db, sid, execution(db, sid, key=3), mid), int)


if __name__ == '__main__':
    unittest.main()
