use super::{PhysicKey, Action, KeyState};
use super::{util, character_map};
use std::collections::HashMap;
use std::iter::FromIterator;

pub struct Vni {
    buffer: Vec<char>
}

const TRIGGER_ACUTE: char = '1';
const TRIGGER_GRAVE: char = '2';
const TRIGGER_HOOK_ABOVE: char = '3';
const TRIGGER_TILDE: char = '4';
const TRIGGER_DOT: char = '5';

const TRIGGER_CIRCUMFLEX: char = '6';
const TRIGGER_HORN: char = '7';
const TRIGGER_BREVE: char = '8';
const TRIGGER_CROSSED_D: char = '9';

struct DiacriticMatch {
    pub ch: char,
    pub pair_with: Vec<char>,
    pub replace_with: (char, char), // lowercase && uppercase
}

impl Vni {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new()
        }
    }

    fn replace_char_at(&mut self, index: usize, ch: char, is_first_edit: bool)
                       -> Vec<Action> {
        let buffer_len = self.buffer.len();
        let mut backspace_amount = buffer_len - index;
        if is_first_edit {
            backspace_amount += 1;
        }
        let deleted_chars = self.buffer
            .iter()
            .skip(index + 1)
            .take(backspace_amount)
            .copied()
            .collect::<Vec<char>>();
        let mut steps: Vec<Action> = vec![
            Action::Backspace(backspace_amount),
            Action::Insert(ch),
        ];
        for deleted_char in deleted_chars {
            steps.push(Action::Insert(deleted_char));
        }
        steps
    }

    /// Add diacritic (6789)
    /// 
    /// Loop through a list of predefined diacritic match
    /// which contains a char to match against and a list
    /// of chars to pair with that char. So for example
    /// if user type au6, it will change to âu. However
    /// if user type aq6, it will not do anything because
    /// q is not in a list to pair with a
    ///
    /// return a list of actions to send to keyboard
    fn add_diacritic(&mut self, matches: Vec<DiacriticMatch>) -> Vec<Action> {
        let buffer_len = self.buffer.len();
        let mut steps: Vec<Action> = Vec::new();
        let mut is_first_match = true;
        for i in 0..buffer_len {
            let ch = self.buffer[i];
            let next_ch = if i + 1 == buffer_len {
                self.buffer[i]
            } else {
                self.buffer[i + 1]
            };
            let clean_ch = util::clean_char(ch);
            for diacritic_match in &matches {
                if diacritic_match.ch == clean_ch.to_ascii_lowercase() {
                    let next_ch_lower = &util::clean_char(
                        next_ch.to_ascii_lowercase()
                    );
                    if diacritic_match.pair_with.contains(next_ch_lower)
                        || i + 1 == buffer_len {
                        let replace_char = if ch.is_ascii_uppercase() {
                            diacritic_match.replace_with.1
                        } else {
                            diacritic_match.replace_with.0
                        };
                        steps = [
                            steps,
                            self.replace_char_at(i, replace_char, is_first_match)
                        ].concat();
                        self.buffer[i] = replace_char;
                        if is_first_match {
                            is_first_match = false;
                        }
                    }
                }
            }
        }
        steps
    }

    // Get the vowel to put the accent on
    //
    // The rule:
    // - If there's ơ put on top of it
    // - otherwise if there's anything with diacritic (hat) put on top of it
    // - otherwise if there's anything go with such as
    //  - oa oe oo oy then put on top of it
    // - otherwise if there's gi then put on top of the next char
    // - otherwise what vowel come first, put it on
    //  - a e i o u y
    fn get_vowel_for_accent(&self) -> Option<(char, usize)> {
        let buffer_len = self.buffer.len();
        let diacritic_chars = ['ê', 'â', 'ô', 'ă', 'ư', 'Ê', 'Â', 'Ô', 'Ă', 'Ư'];
        let pair_with_o_chars = ['a', 'e', 'o', 'y', 'A', 'E', 'O', 'Y'];
        let mut vowel_positions = HashMap::new();
        vowel_positions.insert('a', 5);
        vowel_positions.insert('e', 4);
        vowel_positions.insert('i', 3);
        vowel_positions.insert('o', 2);
        vowel_positions.insert('u', 1);
        vowel_positions.insert('y', 0);

        vowel_positions.insert('A', 5);
        vowel_positions.insert('E', 4);
        vowel_positions.insert('I', 3);
        vowel_positions.insert('O', 2);
        vowel_positions.insert('U', 1);
        vowel_positions.insert('Y', 0);
        let mut max_vowel_position = -1;
        let mut max_vowel_index = 0;
        let mut result_vowel = None;
        for (idx, &ch) in self.buffer.iter().enumerate() {
            let ch_no_accent = util::remove_accents(ch);
            if ch_no_accent == 'ơ' || ch_no_accent == 'Ơ' {
                return Some((ch_no_accent, idx));
            } else if diacritic_chars.contains(&ch_no_accent) {
                result_vowel = Some((ch_no_accent, idx));
            } else if ch_no_accent == 'o'
                && idx + 1 < buffer_len
                && pair_with_o_chars.contains(&self.buffer[idx + 1].clone()) {
                let next_ch = self.buffer[idx + 1];
                return Some((next_ch, idx + 1));
            } else if ch_no_accent == 'g' && idx + 2 < buffer_len {
                if self.buffer[idx + 1] == 'i' {
                    let next_ch = self.buffer[idx + 2];
                    return Some((next_ch, idx + 2));
                }
            } else {
                let vowel_position = vowel_positions.get(&ch_no_accent);
                if let Some(&position) = vowel_position {
                    if position > max_vowel_position {
                        max_vowel_position = position;
                        max_vowel_index = idx;
                    }
                }
            }
        }
        if result_vowel != None {
            return result_vowel;
        } else if max_vowel_position >= 0 {
            let ch = self.buffer[max_vowel_index];
            return Some((ch, max_vowel_index));
        }
        None
    }

    fn add_accent(&mut self, map: [(char, char); 24]) -> Vec<Action> {
        let vowel = self.get_vowel_for_accent();
        if let Some(v) = vowel {
            let ch = v.0;
            let index = v.1;
            let iter = map.iter().copied();
            let replace_ch = HashMap::<char, char>::from_iter(iter)[&ch];
            return self.replace_char_at(index, replace_ch, true);
        }
        vec![]
    }

    fn handle_normal_char(&mut self, ch: char) -> Vec<Action> {
        match ch {
            TRIGGER_CIRCUMFLEX => self.add_diacritic(vec![
                DiacriticMatch {
                    ch: 'a',
                    pair_with: vec!['u', 'n', 'm', 'p', 't', 'c', 'y'],
                    replace_with: ('â', 'Â'),
                },
                DiacriticMatch {
                    ch: 'e',
                    pair_with: vec!['u', 'n', 'm', 'p', 't', 'c', 'y'],
                    replace_with: ('ê', 'Ê'),
                },
                DiacriticMatch {
                    ch: 'o',
                    pair_with: vec!['i', 'n', 'm', 'p', 't', 'c', 'y'],
                    replace_with: ('ô', 'Ô'),
                }
            ]),
            TRIGGER_HORN => self.add_diacritic(vec![
                DiacriticMatch {
                    ch: 'u',
                    pair_with: vec!['o', 'i', 'n', 'm', 'a', 'p', 't', 'c'],
                    replace_with: ('ư', 'Ư'),
                },
                DiacriticMatch {
                    ch: 'o',
                    pair_with: vec!['i', 'n', 'm', 'p', 't', 'c', 'y'],
                    replace_with: ('ơ', 'Ơ'),
                }
            ]),
            TRIGGER_BREVE => self.add_diacritic(vec![
                DiacriticMatch {
                    ch: 'a',
                    pair_with: vec!['p', 'n', 'm', 't', 'c'],
                    replace_with: ('ă', 'Ă'),
                }
            ]),
            TRIGGER_CROSSED_D => self.add_diacritic(vec![
                DiacriticMatch {
                    ch: 'd',
                    pair_with: vec!['a', 'c', 'e', 'i', 'm', 'n', 'o', 'p', 't', 'u', 'y'],
                    replace_with: ('đ', 'Đ'),
                }
            ]),
            TRIGGER_ACUTE => self.add_accent(character_map::ACUTE_MAP),
            TRIGGER_GRAVE => self.add_accent(character_map::GRAVE_MAP),
            TRIGGER_HOOK_ABOVE => self.add_accent(character_map::HOOK_ABOVE_MAP),
            TRIGGER_TILDE => self.add_accent(character_map::TILDE_MAP),
            TRIGGER_DOT => self.add_accent(character_map::DOT_MAP),
            _ => Vec::new()
        }
    }

    pub fn handle_key(&mut self, key: PhysicKey) -> Vec<Action> {
        let mut ch: char = key.clone().into();
        let mut actions: Vec<Action> = Vec::new();
        if let KeyState::KeyPress = key.state {
            let mut clear_buffer = false;
            if key.is_arrow() || key.is_whitespace() {
                clear_buffer = true;
            } else if key.is_backspace() {
                self.buffer.pop();
            } else {
                ch = match key.cap {
                    Some(_) => ch.to_ascii_uppercase(),
                    None => ch
                };
                actions = self.handle_normal_char(ch);
            }
            if clear_buffer {
                self.buffer.clear();
            } else if ch != '\0' && actions.is_empty() {
                self.buffer.push(ch);
            }

            println!("{:?}", self.buffer);
        }
        actions
    }
}
