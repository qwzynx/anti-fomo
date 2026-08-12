import os
from datetime import datetime
from dotenv import load_dotenv
from sqlalchemy import create_engine, Column, Integer, String, Text, DateTime, Float, Boolean, UniqueConstraint
from sqlalchemy.orm import declarative_base, sessionmaker
from sqlalchemy.dialects.postgresql import insert as pg_insert
from sqlalchemy.dialects.sqlite import insert as sqlite_insert

load_dotenv()

# Get Database URL from environment variable, fallback to a local SQLite file
# (Postgres is used in deployment; SQLite keeps local dev dependency-free).
DATABASE_URL = os.getenv("DATABASE_URL", "sqlite:///./antifomo.db")
if DATABASE_URL.startswith("postgres://"):
    DATABASE_URL = DATABASE_URL.replace("postgres://", "postgresql://", 1)

connect_args = {"check_same_thread": False} if DATABASE_URL.startswith("sqlite") else {}
engine = create_engine(DATABASE_URL, connect_args=connect_args)
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)
Base = declarative_base()

class DBScrapedItem(Base):
    __tablename__ = "scraped_items"

    id = Column(Integer, primary_key=True, index=True)
    title = Column(String(512), nullable=False)
    source_platform = Column(String(128), nullable=False)
    item_type = Column(String(64), nullable=False)
    url = Column(Text, nullable=False)
    content_text = Column(Text, nullable=True)
    timestamp = Column(DateTime, default=datetime.utcnow)
    discipline = Column(String(128), nullable=True)
    relevance_score = Column(Float, nullable=True)
    location = Column(Text, nullable=True)

    # Composite unique constraint for Title + Source Platform
    __table_args__ = (
        UniqueConstraint('title', 'source_platform', name='uix_title_source'),
    )

class User(Base):
    __tablename__ = "users"

    id = Column(Integer, primary_key=True, index=True)
    email = Column(String(256), unique=True, nullable=False, index=True)
    password_hash = Column(String(256), nullable=False)
    name = Column(String(128), nullable=True)
    major = Column(String(128), default="Software Engineering")
    created_at = Column(DateTime, default=datetime.utcnow)

def init_db():
    Base.metadata.create_all(bind=engine)
    # Lightweight migrations: create_all does not add columns to existing
    # tables, so add newer columns in place (no-op if they already exist).
    from sqlalchemy import text
    with engine.connect() as conn:
        for ddl in (
            "ALTER TABLE scraped_items ADD COLUMN location TEXT",
        ):
            try:
                conn.execute(text(ddl))
                conn.commit()
            except Exception:
                conn.rollback()

def get_db():
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()

def save_scraped_items(db, items):
    """
    Saves a list of scraped items using ON CONFLICT DO UPDATE on the
    (title, source_platform) unique constraint. Works on Postgres and SQLite.
    """
    if not items:
        return

    is_sqlite = db.get_bind().dialect.name == "sqlite"
    insert = sqlite_insert if is_sqlite else pg_insert

    for item in items:
        item_type_val = item['item_type'].value if hasattr(item['item_type'], 'value') else item['item_type']

        # Ensure timestamp is a naive datetime or handle serialization
        ts = item['timestamp']
        if hasattr(ts, 'tzinfo') and ts.tzinfo is not None:
            ts = ts.replace(tzinfo=None)

        stmt = insert(DBScrapedItem).values(
            title=item['title'],
            source_platform=item['source_platform'],
            item_type=item_type_val,
            url=item['url'],
            content_text=item['content_text'],
            timestamp=ts,
            discipline=item['discipline'],
            relevance_score=item['relevance_score'],
            location=item.get('location')
        )

        update_set = {
            'url': stmt.excluded.url,
            'content_text': stmt.excluded.content_text,
            'discipline': stmt.excluded.discipline,
            'relevance_score': stmt.excluded.relevance_score,
            'timestamp': stmt.excluded.timestamp,
            'location': stmt.excluded.location
        }
        if is_sqlite:
            stmt = stmt.on_conflict_do_update(
                index_elements=['title', 'source_platform'], set_=update_set)
        else:
            stmt = stmt.on_conflict_do_update(
                constraint='uix_title_source', set_=update_set)
        db.execute(stmt)
    db.commit()
