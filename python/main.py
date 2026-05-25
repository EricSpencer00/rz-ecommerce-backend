"""Ecommerce backend — Python/FastAPI implementation."""

import sqlite3
from contextlib import contextmanager
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field

app = FastAPI(title="rz-ecommerce-backend (Python)")

DB_PATH = ":memory:"
_conn: sqlite3.Connection | None = None


def get_db() -> sqlite3.Connection:
    global _conn
    if _conn is None:
        _conn = sqlite3.connect(DB_PATH, check_same_thread=False)
        _conn.row_factory = sqlite3.Row
        _init_db(_conn)
    return _conn


def _init_db(conn: sqlite3.Connection):
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS products (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            price_cents INTEGER NOT NULL CHECK(price_cents > 0),
            stock INTEGER NOT NULL CHECK(stock >= 0)
        );
        CREATE TABLE IF NOT EXISTS cart_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            product_id INTEGER NOT NULL,
            quantity INTEGER NOT NULL CHECK(quantity > 0),
            unit_price INTEGER NOT NULL,
            FOREIGN KEY (product_id) REFERENCES products(id)
        );
        CREATE TABLE IF NOT EXISTS orders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            total_cents INTEGER NOT NULL CHECK(total_cents > 0),
            status TEXT NOT NULL
        );
        INSERT OR IGNORE INTO products (id, name, price_cents, stock) VALUES
            (1, 'Laptop', 99900, 10),
            (2, 'Mouse', 2500, 50),
            (3, 'Keyboard', 7500, 30),
            (4, 'Monitor', 34900, 15),
            (5, 'Headphones', 12900, 25);
    """)


class Product(BaseModel):
    id: int
    name: str
    price_cents: int = Field(gt=0)
    stock: int = Field(ge=0)


class CartItem(BaseModel):
    product_id: int
    quantity: int = Field(gt=0)
    unit_price: int


class AddToCartRequest(BaseModel):
    product_id: int
    quantity: int = Field(gt=0)


class Order(BaseModel):
    id: int
    total_cents: int = Field(gt=0)
    status: str


# --- Products ---


@app.get("/products")
def list_products() -> list[Product]:
    db = get_db()
    rows = db.execute("SELECT id, name, price_cents, stock FROM products").fetchall()
    return [Product(id=r[0], name=r[1], price_cents=r[2], stock=r[3]) for r in rows]


@app.get("/products/{product_id}")
def get_product(product_id: int) -> Product:
    db = get_db()
    row = db.execute(
        "SELECT id, name, price_cents, stock FROM products WHERE id = ?", (product_id,)
    ).fetchone()
    if row is None:
        raise HTTPException(status_code=404, detail="Product not found")
    return Product(id=row[0], name=row[1], price_cents=row[2], stock=row[3])


# --- Cart ---


@app.get("/cart")
def view_cart() -> list[CartItem]:
    db = get_db()
    rows = db.execute("SELECT product_id, quantity, unit_price FROM cart_items").fetchall()
    return [CartItem(product_id=r[0], quantity=r[1], unit_price=r[2]) for r in rows]


@app.post("/cart", status_code=201)
def add_to_cart(req: AddToCartRequest) -> CartItem:
    db = get_db()
    row = db.execute(
        "SELECT price_cents, stock FROM products WHERE id = ?", (req.product_id,)
    ).fetchone()
    if row is None:
        raise HTTPException(status_code=404, detail="Product not found")

    price, stock = row[0], row[1]
    if stock < req.quantity:
        raise HTTPException(status_code=409, detail="Insufficient stock")

    db.execute(
        "INSERT INTO cart_items (product_id, quantity, unit_price) VALUES (?, ?, ?)",
        (req.product_id, req.quantity, price),
    )
    db.commit()
    return CartItem(product_id=req.product_id, quantity=req.quantity, unit_price=price)


@app.delete("/cart", status_code=204)
def clear_cart():
    db = get_db()
    db.execute("DELETE FROM cart_items")
    db.commit()


@app.delete("/cart/{product_id}", status_code=204)
def remove_from_cart(product_id: int):
    db = get_db()
    db.execute("DELETE FROM cart_items WHERE product_id = ?", (product_id,))
    db.commit()


# --- Orders ---


@app.get("/orders")
def list_orders() -> list[Order]:
    db = get_db()
    rows = db.execute("SELECT id, total_cents, status FROM orders").fetchall()
    return [Order(id=r[0], total_cents=r[1], status=r[2]) for r in rows]


@app.post("/orders", status_code=201)
def place_order() -> Order:
    db = get_db()
    items = db.execute("SELECT product_id, quantity, unit_price FROM cart_items").fetchall()
    if not items:
        raise HTTPException(status_code=400, detail="Cart is empty")

    total = sum(row[1] * row[2] for row in items)

    for row in items:
        product_id, quantity = row[0], row[1]
        result = db.execute(
            "UPDATE products SET stock = stock - ? WHERE id = ? AND stock >= ?",
            (quantity, product_id, quantity),
        )
        if result.rowcount == 0:
            db.rollback()
            raise HTTPException(status_code=409, detail=f"Insufficient stock for product {product_id}")

    db.execute("INSERT INTO orders (total_cents, status) VALUES (?, 'confirmed')", (total,))
    order_id = db.execute("SELECT last_insert_rowid()").fetchone()[0]
    db.execute("DELETE FROM cart_items")
    db.commit()

    return Order(id=order_id, total_cents=total, status="confirmed")


if __name__ == "__main__":
    import uvicorn
    get_db()
    print("Python server running on http://localhost:8000")
    uvicorn.run(app, host="0.0.0.0", port=8000)
