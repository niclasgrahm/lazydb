CREATE TABLE IF NOT EXISTS lazydb.products (
    id UInt64,
    sku String,
    name String,
    description String,
    category String,
    subcategory String,
    brand String,
    price Float64,
    cost Float64,
    margin Float64,
    stock UInt32,
    min_stock UInt32,
    weight_kg Float64,
    width_cm Float64,
    height_cm Float64,
    depth_cm Float64,
    color String,
    rating Float32,
    review_count UInt32,
    is_active UInt8,
    created_at DateTime DEFAULT now(),
    updated_at DateTime DEFAULT now()
) ENGINE = MergeTree()
ORDER BY id;

INSERT INTO lazydb.products (id, sku, name, description, category, subcategory, brand, price, cost, margin, stock, min_stock, weight_kg, width_cm, height_cm, depth_cm, color, rating, review_count, is_active)
SELECT
    number + 1 AS id,
    concat('SKU-', leftPad(toString(number + 1), 5, '0')) AS sku,
    concat(
        arrayElement(['Premium', 'Basic', 'Pro', 'Ultra', 'Mini', 'Mega', 'Elite', 'Classic'], (number % 8) + 1),
        ' ',
        arrayElement(['Keyboard', 'Mouse', 'Monitor', 'Headphones', 'Webcam', 'Hub', 'Cable', 'Stand', 'Lamp', 'Chair'], (number % 10) + 1)
    ) AS name,
    concat('High quality ', lower(name), ' for everyday use. Model #', toString(number + 1000)) AS description,
    arrayElement(['Electronics', 'Furniture', 'Accessories', 'Audio', 'Lighting'], (number % 5) + 1) AS category,
    arrayElement(['Input Devices', 'Displays', 'Audio', 'Cables', 'Ergonomics', 'Lighting', 'Storage', 'Networking'], (number % 8) + 1) AS subcategory,
    arrayElement(['Logitech', 'Corsair', 'Razer', 'SteelSeries', 'HyperX', 'BenQ', 'Dell', 'Anker', 'Keychron', 'Autonomous'], (number % 10) + 1) AS brand,
    round(19.99 + (number * 7.31 % 480), 2) AS price,
    round((19.99 + (number * 7.31 % 480)) * (0.4 + (number % 30) / 100.0), 2) AS cost,
    round(price - cost, 2) AS margin,
    toUInt32(10 + (number * 13 % 491)) AS stock,
    toUInt32(5 + (number * 3 % 46)) AS min_stock,
    round(0.1 + (number * 0.37 % 15), 2) AS weight_kg,
    round(5 + (number * 1.7 % 60), 1) AS width_cm,
    round(3 + (number * 2.1 % 45), 1) AS height_cm,
    round(2 + (number * 0.9 % 30), 1) AS depth_cm,
    arrayElement(['Black', 'White', 'Silver', 'Space Gray', 'Navy', 'Red', 'Green', 'Rose Gold'], (number % 8) + 1) AS color,
    round(toFloat32(2.5 + (number * 17 % 26) / 10.0), 1) AS rating,
    toUInt32(number * 7 % 1200) AS review_count,
    if(number % 13 = 0, 0, 1) AS is_active
FROM numbers(300);

CREATE TABLE IF NOT EXISTS lazydb.orders (
    id UInt64,
    product_id UInt64,
    customer_id UInt64,
    customer_name String,
    customer_email String,
    quantity UInt32,
    unit_price Float64,
    discount_pct Float32,
    total Float64,
    tax Float64,
    shipping_cost Float64,
    status String,
    payment_method String,
    shipping_country String,
    shipping_city String,
    tracking_number String,
    notes String,
    is_gift UInt8,
    ordered_at DateTime DEFAULT now(),
    shipped_at Nullable(DateTime)
) ENGINE = MergeTree()
ORDER BY id;

INSERT INTO lazydb.orders (id, product_id, customer_id, customer_name, customer_email, quantity, unit_price, discount_pct, total, tax, shipping_cost, status, payment_method, shipping_country, shipping_city, tracking_number, notes, is_gift)
SELECT
    number + 1 AS id,
    toUInt64(1 + (number * 7 % 300)) AS product_id,
    toUInt64(1000 + (number * 13 % 150)) AS customer_id,
    concat(
        arrayElement(['Alice', 'Bob', 'Carol', 'Dave', 'Eve', 'Frank', 'Grace', 'Hank', 'Ivy', 'Jack', 'Kara', 'Leo'], (number % 12) + 1),
        ' ',
        arrayElement(['Smith', 'Johnson', 'Lee', 'Garcia', 'Chen', 'Wilson', 'Taylor', 'Brown', 'Kumar', 'Müller'], (number % 10) + 1)
    ) AS customer_name,
    concat(lower(arrayElement(['alice', 'bob', 'carol', 'dave', 'eve', 'frank', 'grace', 'hank', 'ivy', 'jack', 'kara', 'leo'], (number % 12) + 1)), toString(customer_id), '@example.com') AS customer_email,
    toUInt32(1 + (number * 3 % 10)) AS quantity,
    round(19.99 + ((product_id - 1) * 7.31 % 480), 2) AS unit_price,
    round(toFloat32((number % 4) * 5), 1) AS discount_pct,
    round(unit_price * quantity * (1 - discount_pct / 100.0), 2) AS total,
    round(total * 0.08, 2) AS tax,
    round(4.99 + (number % 5) * 3.0, 2) AS shipping_cost,
    arrayElement(['pending', 'processing', 'shipped', 'delivered', 'returned', 'cancelled'], (number % 6) + 1) AS status,
    arrayElement(['credit_card', 'debit_card', 'paypal', 'bank_transfer', 'crypto'], (number % 5) + 1) AS payment_method,
    arrayElement(['US', 'UK', 'DE', 'FR', 'JP', 'CA', 'AU', 'SE', 'NL', 'BR'], (number % 10) + 1) AS shipping_country,
    arrayElement(['New York', 'London', 'Berlin', 'Paris', 'Tokyo', 'Toronto', 'Sydney', 'Stockholm', 'Amsterdam', 'São Paulo'], (number % 10) + 1) AS shipping_city,
    concat('TRK-', leftPad(toString(number + 1), 8, '0')) AS tracking_number,
    if(number % 7 = 0, 'Gift wrapped', if(number % 11 = 0, 'Fragile — handle with care', '')) AS notes,
    if(number % 7 = 0, 1, 0) AS is_gift
FROM numbers(500);

CREATE VIEW IF NOT EXISTS lazydb.order_summary AS
SELECT
    p.name AS product_name,
    p.category,
    p.brand,
    sum(o.quantity) AS total_quantity,
    sum(o.total) AS total_revenue,
    avg(o.discount_pct) AS avg_discount,
    count() AS order_count
FROM lazydb.orders o
JOIN lazydb.products p ON o.product_id = p.id
GROUP BY p.name, p.category, p.brand;
