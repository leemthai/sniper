use rayon::prelude::*;

// add two equal-length Vec<f64> and store the result in an existing third Vec
#[allow(dead_code)] // Potentially useful vector utility
pub fn add_into_existing(v1: &[f64], v2: &[f64], result: &mut [f64]) {
    // Assert lengths are equal to prevent a panic.
    // The zip method would silently truncate to the shortest vector otherwise.
    debug_assert_eq!(v1.len(), v2.len());
    debug_assert_eq!(v1.len(), result.len());

    // Update `result` in-place
    v1.iter()
        .zip(v2.iter())
        .zip(result.iter_mut())
        .for_each(|((a, b), c)| {
            *c = a + b;
        });
}

pub fn fill_forward_mut<T>(data: &mut [Option<T>], default: T) -> u32
where
    T: Clone,
{
    let mut last_value = Some(default);
    let mut total_replaced: u32 = 0;

    for item in data.iter_mut() {
        if let &mut Some(ref value) = item {
            // Update the last_value with a clone of the current item.
            last_value = Some(value.clone());
        } else {
            // If the item is None, fill it with a clone of the last seen value.
            if let Some(ref last) = last_value {
                *item = Some(last.clone());
                total_replaced += 1;
            }
        }
    }
    total_replaced
}

/// To test if a Vec<Option<T>> contains any None values, the most idiomatic way in Rust is to use the iterator method any() combined with Option::is_none
pub fn has_any_none_elements<T: Sync>(vector: &Vec<Option<T>>) -> bool {
    vector.par_iter().any(|item| item.is_none())
}

/// evaluates whether all elements in a vector are the same and returns a boolean value indicating the result
pub fn are_all_elements_same<T: PartialEq + Clone>(vector: &[T]) -> bool {
    if vector.is_empty() {
        // An empty vector can be considered to have all elements "the same"
        // as there are no differing elements. Adjust this behavior if needed.
        return true;
    }

    let first_element = &vector[0];
    vector.iter().all(|element| element == first_element)
}

pub fn count_none_elements<T: Sync>(vec_of_options: &Vec<Option<T>>) -> usize {
    vec_of_options
        .par_iter()
        .filter(|option| option.is_none())
        .count()
}

// What % of elements within a Vec<Option<T>> are `None`?
pub fn count_pct_none_elements<T: std::marker::Sync>(vec_of_options: &Vec<Option<T>>) -> f64 {
    ((count_none_elements(vec_of_options) as f64) / vec_of_options.len() as f64) * 100.0
}

// rust function to find the index of the last None items in Vec<Option>T>> given that we will be guaranteed to have None values
pub fn find_last_none_index<T>(vec: &[Option<T>]) -> usize {
    vec.iter()
        .rfind(|item| item.is_none())
        .map(|_| {
            // This unwrap is safe because rfind returns Some if a None is found,
            // and we are guaranteed to have None values.
            let index = vec.iter().rev().position(|x| x.is_none()).unwrap();
            vec.len() - 1 - index
        })
        .unwrap() // This unwrap is safe because we are guaranteed to have None values.
}
