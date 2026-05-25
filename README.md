# rz-ecommerce-backend

A simple ecommerce backend implemented in three languages to compare ergonomics, safety guarantees, and development experience.

## Implementations

| Directory | Language | Stack | Lines |
|-----------|----------|-------|-------|
| `resilient/` | Resilient | Actor model + contracts + live blocks | ~200 |
| `rust/` | Rust | axum + SQLite (rusqlite) | ~250 |
| `python/` | Python | FastAPI + SQLite | ~150 |

## What it does

A minimal product catalog + cart + checkout flow:

- **Products**: list, get by ID, search
- **Cart**: add item, remove item, view cart, clear
- **Orders**: place order (validates stock), view order history
- **Inventory**: stock tracking with atomic decrement

## Running each version

### Resilient
```bash
cd resilient
# Requires the `rz` compiler from github.com/EricSpencer00/Resilient
rz main.rz
```

### Rust
```bash
cd rust
cargo run
# Server starts on http://localhost:3000
```

### Python
```bash
cd python
pip install -r requirements.txt
python main.py
# Server starts on http://localhost:8000
```

## Comparison takeaways

| Dimension | Resilient | Rust | Python |
|-----------|-----------|------|--------|
| **Type safety** | Static types + Z3-verified contracts | Static types + borrow checker | Dynamic (runtime errors) |
| **Error handling** | `fails` declarations + `try/catch` | `Result<T, E>` + `?` operator | Exceptions (unchecked) |
| **Concurrency** | Actor model (isolated state) | async/await (tokio) | async/await (uvicorn) |
| **Fault tolerance** | `live {}` blocks auto-retry | Manual retry logic | Manual retry logic |
| **Formal verification** | Z3 proves contracts at compile time | None built-in | None |
| **Ecosystem maturity** | Young (no HTTP/DB libs) | Mature (crates.io) | Mature (PyPI) |
| **Best fit** | Safety-critical logic, embedded | Systems + web backends | Rapid prototyping, scripts |

### Where Resilient shines in this comparison

The business logic layer — inventory validation, order state machines, price calculations — is where Resilient's contracts prove their worth. You can't ship a bug where `quantity < 0` or `total != sum(items)` because the Z3 solver rejects it at compile time.

### Where Resilient struggles

No HTTP server, no database driver, no JSON serialization. For a web backend today, you'd embed Resilient's verified logic as a library called from a Rust or C host. The language is honest about this: it's for the critical path, not the plumbing.
