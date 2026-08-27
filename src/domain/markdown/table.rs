//! Table model and column-width allocation.

use super::inline::Inline;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub headers: Vec<Vec<Inline>>,
    pub rows: Vec<Vec<Vec<Inline>>>,
    pub alignments: Vec<Alignment>,
}

impl Table {
    /// Number of columns, derived from headers and first row.
    pub fn column_count(&self) -> usize {
        self.headers
            .len()
            .max(self.rows.first().map(|r| r.len()).unwrap_or(0))
    }

    /// Compute column widths within the given total terminal width.
    ///
    /// The returned widths are content-only; the rendered frame adds
    /// `3 * col_count + 1` columns for vertical borders and per-cell padding.
    pub fn allocate_column_widths(&self, total_width: usize) -> Vec<usize> {
        let col_count = self.column_count();
        if col_count == 0 {
            return Vec::new();
        }

        let frame_overhead = table_frame_overhead(col_count);
        let available = total_width.saturating_sub(frame_overhead).max(col_count);

        let mut ideal = vec![0usize; col_count];
        let mut min = vec![0usize; col_count];

        for (i, header) in self.headers.iter().enumerate() {
            ideal[i] = ideal[i].max(Inline::text_width(header));
            min[i] = min[i].max(Inline::min_word_width(header));
        }
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                ideal[i] = ideal[i].max(Inline::text_width(cell));
                min[i] = min[i].max(Inline::min_word_width(cell));
            }
        }

        let total_ideal: usize = ideal.iter().sum();
        let mut widths = if total_ideal <= available {
            ideal
        } else if min.iter().sum::<usize>() >= available {
            distribute_table_width(available, &min, &min)
        } else {
            let total_min: usize = min.iter().sum();
            let extra = available - total_min;
            let desire: Vec<usize> = ideal.iter().zip(&min).map(|(i, m)| i - m).collect();
            let mut widths = min.clone();
            let total_desire: usize = desire.iter().sum();
            if total_desire > 0 {
                for i in 0..col_count {
                    widths[i] += extra
                        .saturating_mul(desire[i])
                        .checked_div(total_desire)
                        .unwrap_or(0);
                }
                let assigned: usize = widths.iter().sum();
                let mut remainder = available.saturating_sub(assigned);
                let mut idx = 0usize;
                while remainder > 0 {
                    widths[idx % col_count] += 1;
                    remainder -= 1;
                    idx += 1;
                }
            } else {
                widths = distribute_table_width(available, &min, &min);
            }
            trim_column_widths(&mut widths, available);
            widths
        };

        if total_ideal > available {
            expand_column_widths(&mut widths, available);
        }
        widths
    }

    /// Rendered table width in terminal columns for `widths`.
    pub fn table_frame_width(widths: &[usize]) -> usize {
        table_frame_overhead(widths.len()) + widths.iter().sum::<usize>()
    }
}

/// Vertical borders plus per-cell `space + content + space` padding.
fn table_frame_overhead(col_count: usize) -> usize {
    3 * col_count + 1
}

fn trim_column_widths(widths: &mut [usize], available: usize) {
    while widths.iter().sum::<usize>() > available {
        let Some(max_idx) = widths
            .iter()
            .enumerate()
            .max_by_key(|(_, v)| *v)
            .map(|(i, _)| i)
        else {
            break;
        };
        if widths[max_idx] > 1 {
            widths[max_idx] -= 1;
        } else {
            break;
        }
    }
}

fn expand_column_widths(widths: &mut [usize], available: usize) {
    let sum: usize = widths.iter().sum();
    if sum >= available {
        return;
    }
    let mut extra = available - sum;
    let n = widths.len();
    for (i, width) in widths.iter_mut().enumerate() {
        if extra == 0 {
            break;
        }
        let remaining = n - i;
        let add = extra / remaining;
        *width += add;
        extra -= add;
    }
}

fn distribute_table_width(available: usize, weights: &[usize], floors: &[usize]) -> Vec<usize> {
    let total_weight: usize = weights.iter().sum();
    if total_weight == 0 {
        return floors.iter().map(|_| 1usize).collect();
    }
    let mut out = Vec::with_capacity(weights.len());
    for (w, floor) in weights.iter().zip(floors) {
        let v = (available * w).div_ceil(total_weight).max(*floor).max(1);
        out.push(v);
    }
    // Trim if rounding pushed us over.
    while out.iter().sum::<usize>() > available {
        if let Some(max_idx) = out
            .iter()
            .enumerate()
            .max_by_key(|(_, v)| *v)
            .map(|(i, _)| i)
        {
            if out[max_idx] > 1 {
                out[max_idx] -= 1;
            } else {
                break;
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    None,
    Left,
    Center,
    Right,
}
