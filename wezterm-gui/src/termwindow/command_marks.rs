//! Command status marks, like iTerm2's marks: a green or red bar beside
//! each prompt whose command has finished, from the exit status the shell
//! reports with OSC 133;D. Clicking a mark selects that command's output.

use wezterm_term::{SemanticType, SemanticZone, StableRowIndex};

/// How many rows a prompt may start below the row that was current when
/// the shell started it. Prompts that begin with a blank line write no
/// cells on that first row, so their prompt zone starts on the next.
const MAX_PROMPT_OFFSET: StableRowIndex = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandMark {
    /// The row where the prompt's text starts.
    pub row: StableRowIndex,
    pub status: i32,
}

/// Matches recorded statuses, `(row, status)` sorted by row, to the start
/// rows of prompt zones, sorted. A status whose row no longer starts a
/// prompt, for example after the screen was cleared, gets no mark.
pub fn place_marks(
    statuses: &[(StableRowIndex, i32)],
    prompt_rows: &[StableRowIndex],
) -> Vec<CommandMark> {
    let mut marks = vec![];
    for (idx, &(row, status)) in statuses.iter().enumerate() {
        let next = statuses
            .get(idx + 1)
            .map(|(r, _)| *r)
            .unwrap_or(StableRowIndex::MAX);
        let first = prompt_rows.partition_point(|&p| p < row);
        if let Some(&prompt) = prompt_rows.get(first) {
            if prompt < next && prompt - row <= MAX_PROMPT_OFFSET {
                marks.push(CommandMark {
                    row: prompt,
                    status,
                });
            }
        }
    }
    marks
}

/// Finds the output of the command whose prompt starts at `prompt_row`:
/// from the first output zone after it to the last output zone before
/// `next_prompt_row`. Returns one zone covering that span, or `None` if
/// the command printed nothing.
pub fn output_zone(
    zones: &[SemanticZone],
    prompt_row: StableRowIndex,
    next_prompt_row: Option<StableRowIndex>,
) -> Option<SemanticZone> {
    let mut seen_input = false;
    let mut result: Option<SemanticZone> = None;
    for zone in zones {
        if zone.end_y < prompt_row {
            continue;
        }
        if let Some(next) = next_prompt_row {
            if zone.start_y >= next {
                break;
            }
        }
        match zone.semantic_type {
            SemanticType::Input => seen_input = true,
            // Output that comes before the command line is text that
            // shared a row with the prompt, not the command's output.
            SemanticType::Output if seen_input => match result.as_mut() {
                None => result = Some(*zone),
                Some(found) => {
                    found.end_y = zone.end_y;
                    found.end_x = zone.end_x;
                }
            },
            SemanticType::Prompt if result.is_some() => {
                // A prompt after output with no status of its own, such
                // as one printed by a program the command ran.
                break;
            }
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod test {
    use super::*;

    fn zone(start_y: StableRowIndex, end_y: StableRowIndex, t: SemanticType) -> SemanticZone {
        SemanticZone {
            start_y,
            start_x: 0,
            end_y,
            end_x: 9,
            semantic_type: t,
        }
    }

    #[test]
    fn marks_go_on_prompt_rows() {
        let marks = place_marks(&[(0, 0), (3, 1)], &[0, 3, 6]);
        assert_eq!(
            marks,
            vec![
                CommandMark { row: 0, status: 0 },
                CommandMark { row: 3, status: 1 }
            ]
        );
    }

    #[test]
    fn prompt_starting_with_a_blank_line() {
        // The status was recorded on the blank row above the prompt text.
        assert_eq!(
            place_marks(&[(4, 2)], &[5]),
            vec![CommandMark { row: 5, status: 2 }]
        );
    }

    #[test]
    fn no_mark_when_the_prompt_is_gone() {
        // Cleared screen: the nearest prompt is far below, or belongs to
        // the next status.
        assert_eq!(place_marks(&[(0, 0)], &[20]), vec![]);
        assert_eq!(
            place_marks(&[(0, 0), (2, 1)], &[2]),
            vec![CommandMark { row: 2, status: 1 }]
        );
    }

    #[test]
    fn output_between_prompts() {
        use SemanticType::*;
        let zones = vec![
            zone(0, 0, Prompt),
            zone(0, 0, Input),
            zone(1, 2, Output),
            zone(3, 3, Prompt),
            zone(3, 3, Input),
            zone(4, 4, Output),
            zone(5, 5, Prompt),
        ];
        let z = output_zone(&zones, 0, Some(3)).unwrap();
        assert_eq!((z.start_y, z.end_y), (1, 2));
        let z = output_zone(&zones, 3, Some(5)).unwrap();
        assert_eq!((z.start_y, z.end_y), (4, 4));
        // The last prompt has no output yet.
        assert_eq!(output_zone(&zones, 5, None), None);
    }

    #[test]
    fn output_spanning_input() {
        use SemanticType::*;
        // A command that read input part way through its output.
        let zones = vec![
            zone(0, 0, Prompt),
            zone(0, 0, Input),
            zone(1, 1, Output),
            zone(2, 2, Input),
            zone(3, 4, Output),
            zone(5, 5, Prompt),
        ];
        let z = output_zone(&zones, 0, None).unwrap();
        assert_eq!((z.start_y, z.end_y), (1, 4));
    }

    #[test]
    fn command_with_no_output() {
        use SemanticType::*;
        let zones = vec![zone(0, 0, Prompt), zone(0, 0, Input), zone(1, 1, Prompt)];
        assert_eq!(output_zone(&zones, 0, Some(1)), None);
    }
}
