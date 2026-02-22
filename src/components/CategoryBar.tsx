interface CategoryBarProps {
  categories: string[];
  activeCategory: string | null;
  onCategoryChange: (category: string | null) => void;
}

function CategoryBar({
  categories,
  activeCategory,
  onCategoryChange,
}: CategoryBarProps) {
  return (
    <div data-testid="category-bar" className="flex items-center gap-1 px-4 py-2 border-b border-launcher-border/30 overflow-x-auto">
      <button
        data-testid="category-all"
        onClick={() => onCategoryChange(null)}
        className={`px-3 py-1 rounded-md text-xs font-medium transition-all whitespace-nowrap ${
          activeCategory === null
            ? "bg-launcher-accent text-white"
            : "text-launcher-muted hover:text-launcher-text hover:bg-launcher-hover"
        }`}
      >
        All
      </button>
      {categories.map((cat) => (
        <button
          data-testid={`category-${cat}`}
          key={cat}
          onClick={() =>
            onCategoryChange(cat === activeCategory ? null : cat)
          }
          className={`px-3 py-1 rounded-md text-xs font-medium transition-all whitespace-nowrap ${
            activeCategory === cat
              ? "bg-launcher-accent text-white"
              : "text-launcher-muted hover:text-launcher-text hover:bg-launcher-hover"
          }`}
        >
          {cat}
        </button>
      ))}
    </div>
  );
}

export default CategoryBar;
