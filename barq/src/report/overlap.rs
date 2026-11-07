use crate::MappedSymbol;
use std::collections::BTreeMap;

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Coverage {
    pub known: u128,
    pub shared: u128,
}

/// Half-open ranges from recorded ELF addresses and sizes. Zero-size entries
/// have no known extent. u128 endpoints avoid wrapping near u64::MAX.
pub(super) fn coverage(ranges: impl IntoIterator<Item = (u128, u128)>) -> Coverage {
    let mut events = BTreeMap::<u128, i64>::new();
    for (start, end) in ranges {
        if start < end {
            *events.entry(start).or_default() += 1;
            *events.entry(end).or_default() -= 1;
        }
    }
    let mut result = Coverage::default();
    let mut previous = 0;
    let mut active = 0;
    for (address, delta) in events {
        if active > 0 {
            result.known += address - previous;
        }
        if active > 1 {
            result.shared += address - previous;
        }
        active += delta;
        previous = address;
    }
    result
}

pub(super) fn range(symbol: &MappedSymbol) -> (u128, u128) {
    let start = u128::from(symbol.address);
    (start, start + u128::from(symbol.size))
}

pub(super) struct AddressGroup<'a> {
    pub address: u64,
    pub symbols: Vec<&'a MappedSymbol>,
    pub known_size: u64,
    pub shared_bytes: u128,
    // Index of the other address group, and the known intersection in bytes.
    pub overlaps: Vec<(usize, u128)>,
}

pub(super) fn group_symbols(symbols: &[MappedSymbol]) -> Vec<AddressGroup<'_>> {
    let mut by_address = BTreeMap::<u64, Vec<&MappedSymbol>>::new();
    for symbol in symbols {
        by_address.entry(symbol.address).or_default().push(symbol);
    }
    let mut groups: Vec<_> = by_address
        .into_iter()
        .map(|(address, symbols)| {
            let known_size = symbols.iter().map(|symbol| symbol.size).max().unwrap_or(0);
            AddressGroup {
                address,
                symbols,
                known_size,
                shared_bytes: 0,
                overlaps: Vec::new(),
            }
        })
        .collect();
    let mut shared_ranges = vec![Vec::new(); groups.len()];
    for (index, group) in groups.iter().enumerate() {
        let mut sizes: Vec<_> = group.symbols.iter().map(|symbol| symbol.size).collect();
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        if let Some(&second_size) = sizes.get(1) {
            let start = u128::from(group.address);
            shared_ranges[index].push((start, start + u128::from(second_size)));
        }
    }
    for i in 0..groups.len() {
        let end_i = u128::from(groups[i].address) + u128::from(groups[i].known_size);
        for j in i + 1..groups.len() {
            let start = u128::from(groups[j].address);
            if start >= end_i {
                break;
            }
            let end_j = start + u128::from(groups[j].known_size);
            let end = end_i.min(end_j);
            if start < end {
                let bytes = end - start;
                groups[i].overlaps.push((j, bytes));
                groups[j].overlaps.push((i, bytes));
                shared_ranges[i].push((start, end));
                shared_ranges[j].push((start, end));
            }
        }
    }
    for (group, ranges) in groups.iter_mut().zip(shared_ranges) {
        group.shared_bytes = coverage(ranges).known;
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SymbolKind;

    fn symbol(address: u64, size: u64) -> MappedSymbol {
        MappedSymbol {
            name: format!("symbol_{address}_{size}"),
            address,
            size,
            section: ".text".to_string(),
            kind: SymbolKind::Function,
        }
    }

    #[test]
    fn partial_overlap_counts_unique_and_shared_bytes_once() {
        let symbols = [symbol(0x1000, 16), symbol(0x1008, 16), symbol(0x100a, 2)];
        assert_eq!(
            coverage(symbols.iter().map(range)),
            Coverage {
                known: 24,
                shared: 8
            }
        );
        let groups = group_symbols(&symbols);
        assert_eq!(groups[0].shared_bytes, 8);
        assert_eq!(groups[0].overlaps, [(1, 8), (2, 2)]);
        assert_eq!(groups[1].shared_bytes, 8);
    }

    #[test]
    fn shared_starts_do_not_infer_sizes_for_markers() {
        let symbols = [symbol(10, 8), symbol(10, 8), symbol(10, 0), symbol(12, 0)];
        let groups = group_symbols(&symbols);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].symbols.len(), 3);
        assert_eq!(groups[0].known_size, 8);
        assert_eq!(groups[0].shared_bytes, 8);
        assert!(groups[0].overlaps.is_empty());
        assert_eq!(groups[1].shared_bytes, 0);
        assert_eq!(
            coverage(symbols.iter().map(range)),
            Coverage {
                known: 8,
                shared: 8
            }
        );
    }

    #[test]
    fn touching_ranges_and_zero_sizes_do_not_overlap() {
        let symbols = [symbol(0, 8), symbol(8, 4), symbol(9, 0)];
        assert_eq!(
            coverage(symbols.iter().map(range)),
            Coverage {
                known: 12,
                shared: 0
            }
        );
        assert!(group_symbols(&symbols)
            .iter()
            .all(|g| g.overlaps.is_empty()));
    }

    #[test]
    fn endpoints_do_not_wrap_at_the_largest_address() {
        let symbols = [symbol(u64::MAX - 3, 4), symbol(u64::MAX, 1)];
        assert_eq!(
            coverage(symbols.iter().map(range)),
            Coverage {
                known: 4,
                shared: 1
            }
        );
    }
}
