import os
from datetime import datetime
from dotenv import load_dotenv
from sqlalchemy import create_engine, Column, Integer, String, Text, DateTime, Float, UniqueConstraint
from sqlalchemy.orm import declarative_base, sessionmaker
from sqlalchemy.dialects.postgresql import insert

load_dotenv()

# Get Database URL from environment variable, fallback to local default
DATABASE_URL = os.getenv("DATABASE_URL", "postgresql://postgres:postgres@localhost:5432/antifomo")
if DATABASE_URL.startswith("postgres://"):
    DATABASE_URL = DATABASE_URL.replace("postgres://", "postgresql://", 1)

engine = create_engine(DATABASE_URL)
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

    # Composite unique constraint for Title + Source Platform
    __table_args__ = (
        UniqueConstraint('title', 'source_platform', name='uix_title_source'),
    )

def init_db():
    Base.metadata.create_all(bind=engine)

def get_db():
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()

def save_scraped_items(db, items):
    """
    Saves a list of scraped items using PostgreSQL ON CONFLICT DO UPDATE
    based on the (title, source_platform) unique constraint.
    """
    if not items:
        return
        
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
            relevance_score=item['relevance_score']
        )
        
        # On conflict (title, source_platform), update fields
        stmt = stmt.on_conflict_do_update(
            constraint='uix_title_source',
            set_={
                'url': stmt.excluded.url,
                'content_text': stmt.excluded.content_text,
                'discipline': stmt.excluded.discipline,
                'relevance_score': stmt.excluded.relevance_score,
                'timestamp': stmt.excluded.timestamp
            }
        )
        db.execute(stmt)
    db.commit()
