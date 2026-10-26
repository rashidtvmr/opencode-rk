//! Standalone, bounded category-grid allocator for context usage rendering.
//!
//! This module deliberately has no renderer or terminal dependencies. `CategoryId` is
//! the stable identity used by both grid cells and the legend; renderer colors must be
//! looked up from that identity, never from a separately ordered palette.

pub const MAX_GRID_COLUMNS: usize = 20;
pub const MAX_GRID_ROWS: usize = 10;
pub const MAX_GRID_CELLS: usize = MAX_GRID_COLUMNS * MAX_GRID_ROWS;
pub const CATEGORY_COUNT: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CategoryId {
    SystemPrompt = 0,
    SystemTools = 1,
    McpInstructions = 2,
    MemoryFiles = 3,
    Skills = 4,
    Messages = 5,
    FreeSpace = 6,
}

impl CategoryId {
    pub const ALL: [Self; CATEGORY_COUNT] = [
        Self::SystemPrompt,
        Self::SystemTools,
        Self::McpInstructions,
        Self::MemoryFiles,
        Self::Skills,
        Self::Messages,
        Self::FreeSpace,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn key(self) -> &'static str {
        match self {
            Self::SystemPrompt => "system_prompt",
            Self::SystemTools => "system_tools",
            Self::McpInstructions => "mcp_instructions",
            Self::MemoryFiles => "memory_files",
            Self::Skills => "skills",
            Self::Messages => "messages",
            Self::FreeSpace => "free_space",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::SystemPrompt => "System prompt",
            Self::SystemTools => "System tools",
            Self::McpInstructions => "MCP server instructions",
            Self::MemoryFiles => "Memory files",
            Self::Skills => "Skills",
            Self::Messages => "Messages",
            Self::FreeSpace => "Free space",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountBasis {
    Measured,
    Estimated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CategoryTokens {
    pub tokens: u64,
    pub basis: CountBasis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    Complete,
    Partial,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridInput {
    /// `None` means the provider/model capacity was not reported.
    pub capacity_tokens: Option<u64>,
    /// `None` means this category amount was not reported; `Some(0)` is measured zero.
    pub categories: [Option<CategoryTokens>; CATEGORY_COUNT],
    /// `None` means the producer did not report coverage status.
    pub coverage: Option<Coverage>,
    /// Available grid dimensions before the 20-by-10 safety bound.
    pub columns: usize,
    pub rows: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridCell {
    Category(CategoryId),
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridState {
    Allocated,
    CapacityUnreported,
    ZeroCapacity,
    NoVisibleArea,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridAllocation {
    pub columns: usize,
    pub rows: usize,
    pub cells: Vec<GridCell>,
    pub state: GridState,
    pub capacity_tokens: Option<u64>,
    /// Original per-category counts and provenance, including explicit zeros and None.
    pub categories: [Option<CategoryTokens>; CATEGORY_COUNT],
    /// Sum uses u128 so seven valid u64 values cannot wrap.
    pub known_tokens: u128,
    /// Known excess over reported capacity; None means capacity is unreported.
    pub over_capacity_tokens: Option<u128>,
    /// Remainder of reported capacity not explained by any reported category.
    /// This is never labeled or colored as free space.
    pub unaccounted_capacity_tokens: Option<u128>,
    pub coverage: Option<Coverage>,
    /// True when one `Unknown` cell marks incomplete coverage, not a token-sized share.
    pub unknown_is_coverage_marker: bool,
    /// Positive known categories which rounded below the available grid resolution.
    pub omitted_categories: Vec<CategoryId>,
}

/// Percentage with an exact widened multiply. `None` means missing/zero capacity.
pub fn percentage(tokens: u64, capacity_tokens: Option<u64>) -> Option<u128> {
    let capacity = capacity_tokens?;
    (capacity != 0).then(|| (tokens as u128 * 100) / capacity as u128)
}

/// Preserves missing-vs-zero display semantics at the renderer boundary.
pub fn category_tokens_label(value: Option<CategoryTokens>) -> String {
    value.map_or_else(|| "unreported".to_owned(), |value| value.tokens.to_string())
}

/// Proportionally maps reported usage to the visible grid. Unreported amounts stay
/// `Unknown`; known over-capacity is normalized only for drawing and separately flagged.
pub fn allocate(input: GridInput) -> GridAllocation {
    let columns = input.columns.min(MAX_GRID_COLUMNS);
    let rows = input.rows.min(MAX_GRID_ROWS);
    let cell_budget = columns * rows;
    let known_tokens = input
        .categories
        .iter()
        .flatten()
        .map(|value| value.tokens as u128)
        .sum::<u128>();
    let over_capacity_tokens = input.capacity_tokens.map(|capacity| {
        known_tokens.saturating_sub(capacity as u128)
    });
    let over_capacity_tokens = over_capacity_tokens.filter(|tokens| *tokens > 0);
    let unaccounted_capacity_tokens = input.capacity_tokens.and_then(|capacity| {
        (capacity as u128)
            .checked_sub(known_tokens)
            .filter(|remaining| *remaining > 0)
    });
    let missing_categories = CategoryId::ALL
        .into_iter()
        .filter(|category| input.categories[category.index()].is_none())
        .collect::<Vec<_>>();
    let coverage = match input.coverage {
        Some(Coverage::Complete)
            if missing_categories.is_empty()
                && unaccounted_capacity_tokens.is_none()
                && over_capacity_tokens.is_none() =>
        {
            Some(Coverage::Complete)
        }
        Some(_) => Some(Coverage::Partial),
        None => None,
    };

    let mut result = GridAllocation {
        columns,
        rows,
        cells: Vec::with_capacity(cell_budget),
        state: GridState::NoVisibleArea,
        capacity_tokens: input.capacity_tokens,
        categories: input.categories,
        known_tokens,
        over_capacity_tokens,
        unaccounted_capacity_tokens,
        coverage,
        unknown_is_coverage_marker: false,
        omitted_categories: Vec::new(),
    };

    if cell_budget == 0 {
        return result;
    }

    let Some(capacity) = input.capacity_tokens else {
        result.state = GridState::CapacityUnreported;
        result.cells.resize(cell_budget, GridCell::Unknown);
        return result;
    };

    let capacity = capacity as u128;
    let denominator = known_tokens.max(capacity);
    if denominator == 0 {
        result.state = GridState::ZeroCapacity;
        return result;
    }

    result.state = GridState::Allocated;
    let mut weights = [0u128; CATEGORY_COUNT + 1];
    for category in CategoryId::ALL {
        weights[category.index()] = input.categories[category.index()]
            .map_or(0, |value| value.tokens as u128);
    }
    // Only a known capacity remainder is unknown area. It is never inferred as FreeSpace.
    if known_tokens < capacity {
        weights[CATEGORY_COUNT] = capacity - known_tokens;
    }

    let mut counts = [0usize; CATEGORY_COUNT + 1];
    let mut remainders = [0u128; CATEGORY_COUNT + 1];
    let mut assigned = 0usize;
    for index in 0..weights.len() {
        if weights[index] == 0 {
            continue;
        }
        let scaled = weights[index] * cell_budget as u128;
        counts[index] = (scaled / denominator) as usize;
        remainders[index] = scaled % denominator;
        assigned += counts[index];
    }

    // Largest remainder, once per eligible part. The deterministic category order
    // breaks ties; unknown area follows all seven stable category IDs.
    let mut received_remainder = [false; CATEGORY_COUNT + 1];
    for _ in 0..cell_budget.saturating_sub(assigned) {
        let mut best: Option<usize> = None;
        for index in 0..weights.len() {
            if weights[index] == 0 || received_remainder[index] {
                continue;
            }
            if best.map_or(true, |current| remainders[index] > remainders[current]) {
                best = Some(index);
            }
        }
        let Some(index) = best else { break };
        counts[index] += 1;
        received_remainder[index] = true;
    }

    // Reserve the incomplete-coverage marker before emitting any cells, keeping
    // stored category counts identical to the cell stream.
    if result.coverage != Some(Coverage::Complete) && counts[CATEGORY_COUNT] == 0 {
        let mut largest: Option<usize> = None;
        for index in 0..CATEGORY_COUNT {
            if largest.map_or(true, |current| counts[index] > counts[current]) {
                largest = Some(index);
            }
        }
        if let Some(index) = largest {
            if counts[index] > 0 {
                counts[index] -= 1;
                counts[CATEGORY_COUNT] = 1;
                result.unknown_is_coverage_marker = true;
            }
        }
    }

    for category in CategoryId::ALL {
        let index = category.index();
        if input.categories[index].is_some_and(|value| value.tokens > 0) && counts[index] == 0 {
            result.omitted_categories.push(category);
        }
        result
            .cells
            .extend(std::iter::repeat(GridCell::Category(category)).take(counts[index]));
    }
    result
        .cells
        .extend(std::iter::repeat(GridCell::Unknown).take(counts[CATEGORY_COUNT]));

    debug_assert_eq!(result.cells.len(), cell_budget);
    for category in CategoryId::ALL {
        debug_assert_eq!(
            result.cells.iter().filter(|cell| **cell == GridCell::Category(category)).count(),
            counts[category.index()]
        );
    }
    debug_assert_eq!(
        result.cells.iter().filter(|cell| **cell == GridCell::Unknown).count(),
        counts[CATEGORY_COUNT]
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amount(tokens: u64) -> Option<CategoryTokens> {
        Some(CategoryTokens {
            tokens,
            basis: CountBasis::Estimated,
        })
    }

    fn input(
        capacity_tokens: Option<u64>,
        categories: [Option<CategoryTokens>; CATEGORY_COUNT],
        columns: usize,
        rows: usize,
    ) -> GridInput {
        GridInput {
            capacity_tokens,
            categories,
            coverage: Some(Coverage::Partial),
            columns,
            rows,
        }
    }

    fn count_category(allocation: &GridAllocation, category: CategoryId) -> usize {
        allocation
            .cells
            .iter()
            .filter(|cell| **cell == GridCell::Category(category))
            .count()
    }

    fn count_unknown(allocation: &GridAllocation) -> usize {
        allocation
            .cells
            .iter()
            .filter(|cell| **cell == GridCell::Unknown)
            .count()
    }

    #[test]
    fn leaves_unreported_capacity_unknown_instead_of_filling_largest_bucket() {
        let mut categories = [None; CATEGORY_COUNT];
        categories[CategoryId::SystemPrompt.index()] = amount(1);
        let grid = allocate(input(Some(100), categories, 20, 10));
        assert_eq!(count_category(&grid, CategoryId::SystemPrompt), 2);
        assert_eq!(count_unknown(&grid), 198);
        assert!(!grid.unknown_is_coverage_marker);
        assert_eq!(grid.cells.len(), MAX_GRID_CELLS);
    }

    #[test]
    fn partial_coverage_with_exact_capacity_gets_explicit_unknown_marker() {
        let mut categories = [amount(0); CATEGORY_COUNT];
        categories[CategoryId::SystemPrompt.index()] = amount(100);
        let grid = allocate(input(Some(100), categories, 20, 10));
        assert_eq!(grid.coverage, Some(Coverage::Partial));
        assert!(grid.unknown_is_coverage_marker);
        assert_eq!(count_unknown(&grid), 1);
        assert_eq!(grid.cells.len(), MAX_GRID_CELLS);
    }

    #[test]
    fn missing_capacity_keeps_known_count_but_never_normalizes_or_invents_free_space() {
        let mut categories = [None; CATEGORY_COUNT];
        categories[CategoryId::SystemPrompt.index()] = amount(17);
        let grid = allocate(input(None, categories, 20, 10));
        assert_eq!(grid.state, GridState::CapacityUnreported);
        assert_eq!(grid.known_tokens, 17);
        assert_eq!(grid.categories[CategoryId::SystemPrompt.index()].unwrap().tokens, 17);
        assert_eq!(count_category(&grid, CategoryId::SystemPrompt), 0);
        assert_eq!(count_unknown(&grid), MAX_GRID_CELLS);
        assert!(!grid.unknown_is_coverage_marker);
        assert_eq!(grid.unaccounted_capacity_tokens, None);
        assert_eq!(percentage(17, None), None);
    }

    #[test]
    fn aggregate_over_capacity_is_bounded_and_explicit() {
        let mut categories = [Some(CategoryTokens {
            tokens: 0,
            basis: CountBasis::Measured,
        }); CATEGORY_COUNT];
        categories[CategoryId::SystemPrompt.index()] = amount(80);
        categories[CategoryId::Messages.index()] = amount(80);
        let grid = allocate(input(Some(100), categories, 20, 10));
        assert_eq!(grid.over_capacity_tokens, Some(60));
        assert_eq!(grid.cells.len(), MAX_GRID_CELLS);
        assert_eq!(count_category(&grid, CategoryId::SystemPrompt), 99);
        assert_eq!(count_category(&grid, CategoryId::Messages), 100);
        assert!(grid.unknown_is_coverage_marker);
        assert_eq!(count_unknown(&grid), 1);
        let known_cells = CategoryId::ALL.into_iter()
            .map(|category| count_category(&grid, category)).sum::<usize>();
        assert_eq!(known_cells + count_unknown(&grid), grid.cells.len());
    }

    #[test]
    fn narrow_grid_rescales_visible_cells_and_reports_unrendered_positive_category() {
        let mut categories = [None; CATEGORY_COUNT];
        categories[CategoryId::SystemPrompt.index()] = amount(1);
        categories[CategoryId::Messages.index()] = amount(999);
        let grid = allocate(input(Some(1_000), categories, 8, 10));
        assert_eq!(grid.columns, 8);
        assert_eq!(grid.rows, 10);
        assert_eq!(grid.cells.len(), 80);
        assert!(grid.omitted_categories.contains(&CategoryId::SystemPrompt));
        assert_eq!(grid.omitted_categories, [CategoryId::SystemPrompt]);
    }

    #[test]
    fn percentage_uses_wide_arithmetic_for_u64_maximum() {
        assert_eq!(percentage(u64::MAX, Some(u64::MAX)), Some(100));
        assert_eq!(percentage(u64::MAX, Some(1)), Some(u64::MAX as u128 * 100));
        assert_eq!(percentage(0, Some(0)), None);
    }

    #[test]
    fn category_id_and_legend_order_are_the_same_stable_identity_sequence() {
        for (index, category) in CategoryId::ALL.into_iter().enumerate() {
            assert_eq!(category.index(), index);
            assert!(!category.key().is_empty());
            assert!(!category.label().is_empty());
        }
        assert_eq!(CategoryId::ALL[6], CategoryId::FreeSpace);
    }

    #[test]
    fn category_labels_distinguish_unreported_from_explicit_zero() {
        assert_eq!(category_tokens_label(None), "unreported");
        assert_eq!(category_tokens_label(amount(0)), "0");
        assert_eq!(category_tokens_label(amount(9)), "9");
    }

    #[test]
    fn explicit_zero_is_distinct_from_an_unreported_category() {
        let mut categories = [None; CATEGORY_COUNT];
        categories[CategoryId::SystemPrompt.index()] = amount(0);
        let grid = allocate(input(Some(100), categories, 20, 10));
        assert_eq!(grid.categories[CategoryId::SystemPrompt.index()].unwrap().tokens, 0);
        assert_eq!(grid.categories[CategoryId::Messages.index()], None);
        assert_eq!(percentage(0, Some(100)), Some(0));
        assert_eq!(percentage(0, None), None);
        assert_eq!(grid.unaccounted_capacity_tokens, Some(100));
        assert_eq!(count_unknown(&grid), MAX_GRID_CELLS);
    }

    #[test]
    fn sums_seven_maximum_u64_counts_without_wrapping() {
        let categories = [amount(u64::MAX); CATEGORY_COUNT];
        let grid = allocate(input(Some(u64::MAX), categories, 20, 10));
        assert_eq!(grid.known_tokens, u64::MAX as u128 * CATEGORY_COUNT as u128);
        assert_eq!(grid.over_capacity_tokens, Some(u64::MAX as u128 * 6));
        assert_eq!(grid.cells.len(), MAX_GRID_CELLS);
        assert!(grid.unknown_is_coverage_marker);
        assert_eq!(count_unknown(&grid), 1);
        let known_cells = CategoryId::ALL.into_iter()
            .map(|category| count_category(&grid, category)).sum::<usize>();
        assert_eq!(known_cells + count_unknown(&grid), grid.cells.len());
    }

    #[test]
    fn zero_dimensions_and_large_dimensions_remain_bounded() {
        let empty = allocate(input(Some(100), [amount(100); CATEGORY_COUNT], 0, 10));
        assert_eq!(empty.state, GridState::NoVisibleArea);
        assert!(empty.cells.is_empty());

        let bounded = allocate(input(Some(100), [amount(100); CATEGORY_COUNT], usize::MAX, usize::MAX));
        assert_eq!(bounded.columns, MAX_GRID_COLUMNS);
        assert_eq!(bounded.rows, MAX_GRID_ROWS);
        assert_eq!(bounded.cells.len(), MAX_GRID_CELLS);
    }
}
