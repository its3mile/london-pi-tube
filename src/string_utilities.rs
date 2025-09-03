pub fn insert_linebreaks_inplace<const N: usize>(s: &mut heapless::String<N>, max_line_len: usize) {
    let mut i = 0;
    while i < s.len() {
        // Find the end of the current line (either '\n' or end of string)
        let line_end = match s[i..].find('\n') {
            Some(rel) => i + rel,
            None => s.len(),
        };
        // Only insert a break if the line is too long
        if line_end - i > max_line_len {
            // Look for the last space before the limit
            let mut break_pos = None;
            for j in (i..i + max_line_len).rev() {
                if j < s.len() && s.as_bytes()[j] == b' ' {
                    break_pos = Some(j);
                    break;
                }
            }
            let insert_at = break_pos.unwrap_or(i + max_line_len);
            if s.len() < s.capacity() {
                let mut tail = heapless::String::<N>::new();
                let _ = tail.push_str(&s[insert_at..]);
                let _ = s.truncate(insert_at);
                let _ = s.push('\n');
                let _ = s.push_str(&tail);
                // Move i to after the inserted line break
                i = insert_at + 1;
            } else {
                break;
            }
        } else {
            // Move i to the next line (after '\n' if present)
            i = if line_end < s.len() { line_end + 1 } else { s.len() };
        }
    }
}

pub fn extract_first_json_object(body: &[u8]) -> Option<&[u8]> {
    let mut start = None;
    let mut end = None;
    let mut brace_count = 0;

    for (i, &b) in body.iter().enumerate() {
        if b == b'{' {
            if start.is_none() {
                start = Some(i);
            }
            brace_count += 1;
        }
        if b == b'}' && start.is_some() {
            brace_count -= 1;
            if brace_count == 0 {
                end = Some(i);
                break;
            }
        }
    }

    match (start, end) {
        (Some(s), Some(e)) => Some(&body[s..=e]), // inclusive of the closing brace
        _ => None,
    }
}

pub fn is_empty_json_array(body: &[u8]) -> bool {
    if body.len() == 2 && body[0] == b'[' && body[1] == b']' {
        true
    } else {
        false
    }
}

pub fn first_two_words(s: &str) -> &str {
    let mut space_count = 0;
    for (i, c) in s.char_indices() {
        if c == ' ' {
            space_count += 1;
            if space_count == 2 {
                return &s[..i];
            }
        }
    }
    s
}

pub fn split_iso8601_timestamp(s: &str) -> (&str, &str) {
    let date_end = match s.find('T') {
        Some(i) => i,
        None => s.len(),
    };
    let time_end_wo_frac = match s.find('.') {
        Some(i) => i,
        None => s.len(),
    };
    let date = &s[..date_end];
    let time = &s[date_end + 1..time_end_wo_frac];
    (date, time)
}
