use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, delete},
    Router,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Product {
    id: i64,
    name: String,
    price_cents: i64,
    stock: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CartItem {
    product_id: i64,
    quantity: i64,
    unit_price: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Order {
    id: i64,
    total_cents: i64,
    status: String,
}

#[derive(Debug, Deserialize)]
struct AddToCartRequest {
    product_id: i64,
    quantity: i64,
}

type AppState = Arc<Mutex<Connection>>;

fn init_db(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS products (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            price_cents INTEGER NOT NULL CHECK(price_cents > 0),
            stock INTEGER NOT NULL CHECK(stock >= 0)
        );
        CREATE TABLE IF NOT EXISTS cart_items (
            id INTEGER PRIMARY KEY,
            product_id INTEGER NOT NULL,
            quantity INTEGER NOT NULL CHECK(quantity > 0),
            unit_price INTEGER NOT NULL,
            FOREIGN KEY (product_id) REFERENCES products(id)
        );
        CREATE TABLE IF NOT EXISTS orders (
            id INTEGER PRIMARY KEY,
            total_cents INTEGER NOT NULL CHECK(total_cents > 0),
            status TEXT NOT NULL
        );
        INSERT OR IGNORE INTO products (id, name, price_cents, stock) VALUES
            (1, 'Laptop', 99900, 10),
            (2, 'Mouse', 2500, 50),
            (3, 'Keyboard', 7500, 30),
            (4, 'Monitor', 34900, 15),
            (5, 'Headphones', 12900, 25);",
    )
    .expect("failed to initialize database");
}

async fn list_products(State(db): State<AppState>) -> Json<Vec<Product>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT id, name, price_cents, stock FROM products")
        .unwrap();
    let products: Vec<Product> = stmt
        .query_map([], |row| {
            Ok(Product {
                id: row.get(0)?,
                name: row.get(1)?,
                price_cents: row.get(2)?,
                stock: row.get(3)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    Json(products)
}

async fn get_product(
    State(db): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Product>, StatusCode> {
    let conn = db.lock().unwrap();
    conn.query_row(
        "SELECT id, name, price_cents, stock FROM products WHERE id = ?1",
        [id],
        |row| {
            Ok(Product {
                id: row.get(0)?,
                name: row.get(1)?,
                price_cents: row.get(2)?,
                stock: row.get(3)?,
            })
        },
    )
    .map(Json)
    .map_err(|_| StatusCode::NOT_FOUND)
}

async fn add_to_cart(
    State(db): State<AppState>,
    Json(req): Json<AddToCartRequest>,
) -> Result<Json<CartItem>, StatusCode> {
    let conn = db.lock().unwrap();
    let product: Product = conn
        .query_row(
            "SELECT id, name, price_cents, stock FROM products WHERE id = ?1",
            [req.product_id],
            |row| {
                Ok(Product {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    price_cents: row.get(2)?,
                    stock: row.get(3)?,
                })
            },
        )
        .map_err(|_| StatusCode::NOT_FOUND)?;

    if product.stock < req.quantity {
        return Err(StatusCode::CONFLICT);
    }

    conn.execute(
        "INSERT INTO cart_items (product_id, quantity, unit_price) VALUES (?1, ?2, ?3)",
        (req.product_id, req.quantity, product.price_cents),
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(CartItem {
        product_id: req.product_id,
        quantity: req.quantity,
        unit_price: product.price_cents,
    }))
}

async fn view_cart(State(db): State<AppState>) -> Json<Vec<CartItem>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT product_id, quantity, unit_price FROM cart_items")
        .unwrap();
    let items: Vec<CartItem> = stmt
        .query_map([], |row| {
            Ok(CartItem {
                product_id: row.get(0)?,
                quantity: row.get(1)?,
                unit_price: row.get(2)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    Json(items)
}

async fn clear_cart(State(db): State<AppState>) -> StatusCode {
    let conn = db.lock().unwrap();
    conn.execute("DELETE FROM cart_items", []).unwrap();
    StatusCode::NO_CONTENT
}

async fn remove_from_cart(
    State(db): State<AppState>,
    Path(product_id): Path<i64>,
) -> StatusCode {
    let conn = db.lock().unwrap();
    conn.execute(
        "DELETE FROM cart_items WHERE product_id = ?1",
        [product_id],
    )
    .unwrap();
    StatusCode::NO_CONTENT
}

async fn place_order(State(db): State<AppState>) -> Result<Json<Order>, StatusCode> {
    let conn = db.lock().unwrap();

    let mut stmt = conn
        .prepare("SELECT product_id, quantity, unit_price FROM cart_items")
        .unwrap();
    let items: Vec<CartItem> = stmt
        .query_map([], |row| {
            Ok(CartItem {
                product_id: row.get(0)?,
                quantity: row.get(1)?,
                unit_price: row.get(2)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    if items.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let total: i64 = items.iter().map(|i| i.quantity * i.unit_price).sum();

    for item in &items {
        let updated = conn.execute(
            "UPDATE products SET stock = stock - ?1 WHERE id = ?2 AND stock >= ?1",
            (item.quantity, item.product_id),
        );
        match updated {
            Ok(0) => return Err(StatusCode::CONFLICT),
            Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
            _ => {}
        }
    }

    conn.execute(
        "INSERT INTO orders (total_cents, status) VALUES (?1, 'confirmed')",
        [total],
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let order_id = conn.last_insert_rowid();
    conn.execute("DELETE FROM cart_items", []).unwrap();

    Ok(Json(Order {
        id: order_id,
        total_cents: total,
        status: "confirmed".to_string(),
    }))
}

async fn list_orders(State(db): State<AppState>) -> Json<Vec<Order>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT id, total_cents, status FROM orders")
        .unwrap();
    let orders: Vec<Order> = stmt
        .query_map([], |row| {
            Ok(Order {
                id: row.get(0)?,
                total_cents: row.get(1)?,
                status: row.get(2)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    Json(orders)
}

#[tokio::main]
async fn main() {
    let conn = Connection::open(":memory:").expect("failed to open database");
    init_db(&conn);
    let state: AppState = Arc::new(Mutex::new(conn));

    let app = Router::new()
        .route("/products", get(list_products))
        .route("/products/{id}", get(get_product))
        .route("/cart", get(view_cart).post(add_to_cart).delete(clear_cart))
        .route("/cart/{product_id}", delete(remove_from_cart))
        .route("/orders", get(list_orders).post(place_order))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Rust server running on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
