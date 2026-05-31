-- Demo seed for local UI walkthrough. 5 realistic outdoor products with
-- multiple store listings each, so the price-comparison UI has data.
-- Idempotent: re-running deletes the prior demo rows (matched by id prefix)
-- before re-inserting.
--
-- Live Redis prices are seeded separately by scripts/local_demo.sh because
-- they need a fresh `fetched_at` timestamp (the API marks anything older
-- than ~25h as stale).

BEGIN;

DELETE FROM store_listings WHERE id::text LIKE '22222222-%';
DELETE FROM products       WHERE id::text LIKE '11111111-%';

INSERT INTO products (id, slug, name, brand, category, subcategory, description, primary_image, canonical_key) VALUES
  ('11111111-0000-0000-0000-000000000001',
   'msr-pocketrocket-2', 'PocketRocket 2', 'MSR',
   'camping-cookware', 'stoves',
   'Ultralight 2.6 oz canister stove. Boils 1 L in 3.5 minutes. Folds to fit inside a small pot.',
   'https://placehold.co/600x600/2d3748/ffffff?text=MSR+PocketRocket+2',
   'msr:pocketrocket-2'),

  ('11111111-0000-0000-0000-000000000002',
   'garmin-fenix-7', 'Fenix 7', 'Garmin',
   'electronics', 'gps-watches',
   'Multisport GPS watch with solar charging, AMOLED display, and built-in topo maps.',
   'https://placehold.co/600x600/2d3748/ffffff?text=Garmin+Fenix+7',
   'garmin:fenix-7'),

  ('11111111-0000-0000-0000-000000000003',
   'patagonia-down-sweater', 'Down Sweater Jacket', 'Patagonia',
   'apparel', 'insulated-jackets',
   '800-fill-power recycled down. Windproof, water-repellent shell. 13.1 oz.',
   'https://placehold.co/600x600/2d3748/ffffff?text=Patagonia+Down+Sweater',
   'patagonia:down-sweater'),

  ('11111111-0000-0000-0000-000000000004',
   'black-diamond-spot-400', 'Spot 400 Headlamp', 'Black Diamond',
   'lighting', 'headlamps',
   '400-lumen waterproof (IPX8) headlamp with PowerTap dimming and red-night vision mode.',
   'https://placehold.co/600x600/2d3748/ffffff?text=Black+Diamond+Spot+400',
   'blackdiamond:spot-400'),

  ('11111111-0000-0000-0000-000000000005',
   'osprey-atmos-ag-65', 'Atmos AG 65', 'Osprey',
   'backpacks', 'backpacking-packs',
   '65 L backpacking pack with Anti-Gravity suspension. Fits 4-7 day trips. 4 lb 9 oz.',
   'https://placehold.co/600x600/2d3748/ffffff?text=Osprey+Atmos+AG+65',
   'osprey:atmos-ag-65');

-- 17 store_listings across 6 distinct stores — gives the price-comparison
-- table real cross-store data to render.
INSERT INTO store_listings (id, product_id, store_id, store_product_id, store_url, store_rating, store_review_count, match_confidence, last_synced_at) VALUES
  -- MSR PocketRocket 2 → amazon, rei, campsaver
  ('22222222-0001-0001-0000-000000000000', '11111111-0000-0000-0000-000000000001', 'amazon',    'B07JJ5JDXF',  'https://www.amazon.com/dp/B07JJ5JDXF',                4.70, 1284, 'EXACT', NOW()),
  ('22222222-0001-0002-0000-000000000000', '11111111-0000-0000-0000-000000000001', 'rei',       '131055',      'https://www.rei.com/product/131055/msr-pocketrocket-2', 4.60,  892, 'HIGH',  NOW()),
  ('22222222-0001-0003-0000-000000000000', '11111111-0000-0000-0000-000000000001', 'campsaver', 'MSR-PR2',     'https://www.campsaver.com/pocket-rocket-2-stove',     4.50,  121, 'HIGH',  NOW()),

  -- Garmin Fenix 7 → amazon, rei, backcountry
  ('22222222-0002-0001-0000-000000000000', '11111111-0000-0000-0000-000000000002', 'amazon',      'B09KGJTM1Q', 'https://www.amazon.com/dp/B09KGJTM1Q',                       4.50, 3142, 'EXACT', NOW()),
  ('22222222-0002-0002-0000-000000000000', '11111111-0000-0000-0000-000000000002', 'rei',         '195412',     'https://www.rei.com/product/195412/garmin-fenix-7',          4.40,  421, 'HIGH',  NOW()),
  ('22222222-0002-0004-0000-000000000000', '11111111-0000-0000-0000-000000000002', 'backcountry', 'GMN0142',    'https://www.backcountry.com/garmin-fenix-7',                 4.30,  87,  'HIGH',  NOW()),

  -- Patagonia Down Sweater → rei, backcountry, moosejaw
  ('22222222-0003-0002-0000-000000000000', '11111111-0000-0000-0000-000000000003', 'rei',         '142345', 'https://www.rei.com/product/142345/patagonia-down-sweater',    4.80, 2103, 'EXACT', NOW()),
  ('22222222-0003-0004-0000-000000000000', '11111111-0000-0000-0000-000000000003', 'backcountry', 'PAT0501', 'https://www.backcountry.com/patagonia-down-sweater-jacket',   4.70,  541, 'HIGH',  NOW()),
  ('22222222-0003-0005-0000-000000000000', '11111111-0000-0000-0000-000000000003', 'moosejaw',    'MJ-PAT-DS', 'https://www.moosejaw.com/product/patagonia-down-sweater',  4.60,  302, 'HIGH',  NOW()),

  -- Black Diamond Spot 400 → amazon, rei, campsaver, garagerowngear
  ('22222222-0004-0001-0000-000000000000', '11111111-0000-0000-0000-000000000004', 'amazon',         'B0B5L1Q2KT', 'https://www.amazon.com/dp/B0B5L1Q2KT',                            4.60, 4521, 'EXACT', NOW()),
  ('22222222-0004-0002-0000-000000000000', '11111111-0000-0000-0000-000000000004', 'rei',            '198765',     'https://www.rei.com/product/198765/black-diamond-spot-400',       4.70, 1230, 'HIGH',  NOW()),
  ('22222222-0004-0003-0000-000000000000', '11111111-0000-0000-0000-000000000004', 'campsaver',      'BD-SPOT400', 'https://www.campsaver.com/black-diamond-spot-400-headlamp',       4.50,  89,  'HIGH',  NOW()),
  ('22222222-0004-0008-0000-000000000000', '11111111-0000-0000-0000-000000000004', 'garagerowngear', 'GGG-BD-SPOT', 'https://www.garagegrowngear.com/products/bd-spot-400-headlamp',  4.60,  44,  'HIGH',  NOW()),

  -- Osprey Atmos AG 65 → amazon, rei, backcountry, moosejaw
  ('22222222-0005-0001-0000-000000000000', '11111111-0000-0000-0000-000000000005', 'amazon',      'B07GFKJX42', 'https://www.amazon.com/dp/B07GFKJX42',                          4.70, 5210, 'EXACT', NOW()),
  ('22222222-0005-0002-0000-000000000000', '11111111-0000-0000-0000-000000000005', 'rei',         '120456',     'https://www.rei.com/product/120456/osprey-atmos-ag-65',         4.80, 2841, 'HIGH',  NOW()),
  ('22222222-0005-0004-0000-000000000000', '11111111-0000-0000-0000-000000000005', 'backcountry', 'OSP1023',    'https://www.backcountry.com/osprey-atmos-ag-65',                4.70,  612, 'HIGH',  NOW()),
  ('22222222-0005-0005-0000-000000000000', '11111111-0000-0000-0000-000000000005', 'moosejaw',    'MJ-OSP-AT65', 'https://www.moosejaw.com/product/osprey-atmos-ag-65',          4.60,  398, 'HIGH',  NOW());

COMMIT;
