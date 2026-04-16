CREATE TABLE IF NOT EXISTS lazydb.products (
    id UInt64,
    name String,
    category String,
    price Float64,
    stock UInt32,
    created_at DateTime DEFAULT now()
) ENGINE = MergeTree()
ORDER BY id;

INSERT INTO lazydb.products (id, name, category, price, stock) VALUES
    (1, 'Mechanical Keyboard', 'Electronics', 89.99, 150),
    (2, 'USB-C Hub', 'Electronics', 34.50, 300),
    (3, 'Standing Desk', 'Furniture', 449.00, 45),
    (4, 'Monitor Arm', 'Furniture', 129.99, 80),
    (5, 'Noise Cancelling Headphones', 'Electronics', 249.00, 200),
    (6, 'Ergonomic Mouse', 'Electronics', 59.99, 175),
    (7, 'Desk Lamp', 'Furniture', 42.00, 90),
    (8, 'Webcam HD', 'Electronics', 79.00, 120);

CREATE TABLE IF NOT EXISTS lazydb.orders (
    id UInt64,
    product_id UInt64,
    quantity UInt32,
    total Float64,
    ordered_at DateTime DEFAULT now()
) ENGINE = MergeTree()
ORDER BY id;

INSERT INTO lazydb.orders (id, product_id, quantity, total) VALUES
    (1, 1, 2, 179.98),
    (2, 3, 1, 449.00),
    (3, 5, 3, 747.00),
    (4, 2, 5, 172.50),
    (5, 6, 1, 59.99);

CREATE VIEW IF NOT EXISTS lazydb.order_summary AS
SELECT
    p.name AS product_name,
    p.category,
    sum(o.quantity) AS total_quantity,
    sum(o.total) AS total_revenue
FROM lazydb.orders o
JOIN lazydb.products p ON o.product_id = p.id
GROUP BY p.name, p.category;
