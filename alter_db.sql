CREATE TYPE status AS ENUM (
    'initiated'
    'bridged',
    'in_progress',
    'claim_ready'
    );

ALTER TABLE bridge_event
    ALTER COLUMN status TYPE status
        USING status::status;
ALTER TABLE public.avail_indexer
    ALTER COLUMN signature_address SET NOT NULL;