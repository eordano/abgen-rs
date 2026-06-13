-- Stratified entity-sampling SQL for the test + validation corpora.
-- Run against any Decentraland catalyst `content` database (must expose
-- the entity_type, deployments, content_files schema).
--
--   psql -h <host> -U <user> -d content -t -A -F'\t' -f _regen.sql
--
-- Change the seed suffix (`val` / `test`) to draw a disjoint sample.
-- Force-include known interesting entities (draco etc.) via the union tail.

WITH scenes AS (
    SELECT d.entity_id, COUNT(c.content_hash) fc,
           NTILE(10) OVER (ORDER BY COUNT(c.content_hash)) bucket
    FROM deployments d JOIN content_files c ON c.deployment = d.id
    WHERE d.deleter_deployment IS NULL AND d.entity_type = 'scene'
    GROUP BY d.id, d.entity_id),
scene_sample AS (
    SELECT entity_id, 'scene' entity_type, fc FROM (
        SELECT entity_id, fc, bucket,
               ROW_NUMBER() OVER (PARTITION BY bucket ORDER BY md5(entity_id || 'val')) rn
        FROM scenes) s WHERE rn <= 20),
wearable_sample AS (
    SELECT d.entity_id, 'wearable' entity_type, COUNT(c.content_hash) fc
    FROM deployments d JOIN content_files c ON c.deployment = d.id
    WHERE d.deleter_deployment IS NULL AND d.entity_type = 'wearable'
    GROUP BY d.id, d.entity_id ORDER BY md5(d.entity_id || 'val') LIMIT 50),
emote_sample AS (
    SELECT d.entity_id, 'emote' entity_type, COUNT(c.content_hash) fc
    FROM deployments d JOIN content_files c ON c.deployment = d.id
    WHERE d.deleter_deployment IS NULL AND d.entity_type = 'emote'
    GROUP BY d.id, d.entity_id ORDER BY md5(d.entity_id || 'val') LIMIT 30),
profile_sample AS (
    SELECT d.entity_id, 'profile' entity_type, COUNT(c.content_hash) fc
    FROM deployments d JOIN content_files c ON c.deployment = d.id
    WHERE d.deleter_deployment IS NULL AND d.entity_type = 'profile'
    GROUP BY d.id, d.entity_id HAVING COUNT(c.content_hash) >= 2
    ORDER BY md5(d.entity_id || 'val') LIMIT 20)
SELECT entity_id, entity_type, fc FROM scene_sample
UNION ALL SELECT entity_id, entity_type, fc FROM wearable_sample
UNION ALL SELECT entity_id, entity_type, fc FROM emote_sample
UNION ALL SELECT entity_id, entity_type, fc FROM profile_sample
ORDER BY entity_type, fc, entity_id;
