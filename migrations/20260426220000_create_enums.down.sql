-- Down: drop enum types for equity module
DROP TYPE IF EXISTS share_txn_type CASCADE;
DROP TYPE IF EXISTS holder_type CASCADE;
DROP TYPE IF EXISTS dividend_status CASCADE;
