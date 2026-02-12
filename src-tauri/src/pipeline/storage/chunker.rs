use super::types::{Chunker, TextChunk};

/// Semantic chunker for medical Markdown documents.
/// Splits by section headings first, then by paragraphs for large sections.
pub struct MedicalChunker {
    max_chunk_chars: usize,
    min_chunk_chars: usize,
    overlap_chars: usize,
}

impl MedicalChunker {
    pub fn new() -> Self {
        Self {
            max_chunk_chars: 1000,
            min_chunk_chars: 20,
            overlap_chars: 100,
        }
    }
}

impl Default for MedicalChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunker for MedicalChunker {
    fn chunk(&self, markdown: &str) -> Vec<TextChunk> {
        let mut chunks = Vec::new();
        let mut chunk_index = 0;

        let sections = split_by_headings(markdown);

        for section in &sections {
            if section.content.len() <= self.max_chunk_chars {
                // Always include sections — merge_tiny_chunks handles small ones
                chunks.push(TextChunk {
                    content: section.content.clone(),
                    chunk_index,
                    section_title: section.title.clone(),
                    char_offset: section.offset,
                });
                chunk_index += 1;
            } else {
                let sub_chunks = split_section_by_paragraphs(
                    &section.content,
                    &section.title,
                    section.offset,
                    self.max_chunk_chars,
                    self.overlap_chars,
                    &mut chunk_index,
                );
                chunks.extend(sub_chunks);
            }
        }

        merge_tiny_chunks(&mut chunks, self.min_chunk_chars);
        chunks
    }
}

struct MarkdownSection {
    title: Option<String>,
    content: String,
    offset: usize,
}

fn split_by_headings(markdown: &str) -> Vec<MarkdownSection> {
    let mut sections = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_content = String::new();
    let mut current_offset = 0;
    let mut char_pos = 0;

    for line in markdown.lines() {
        if line.starts_with("## ") || line.starts_with("### ") {
            if !current_content.trim().is_empty() {
                sections.push(MarkdownSection {
                    title: current_title.take(),
                    content: current_content.trim().to_string(),
                    offset: current_offset,
                });
            }
            current_title = Some(line.trim_start_matches('#').trim().to_string());
            current_content = String::new();
            current_offset = char_pos;
        } else {
            current_content.push_str(line);
            current_content.push('\n');
        }
        char_pos += line.len() + 1;
    }

    if !current_content.trim().is_empty() {
        sections.push(MarkdownSection {
            title: current_title,
            content: current_content.trim().to_string(),
            offset: current_offset,
        });
    }

    sections
}

fn split_section_by_paragraphs(
    content: &str,
    title: &Option<String>,
    base_offset: usize,
    max_chars: usize,
    overlap: usize,
    chunk_index: &mut usize,
) -> Vec<TextChunk> {
    let mut chunks = Vec::new();
    let paragraphs: Vec<&str> = content.split("\n\n").collect();

    let mut current = String::new();
    let mut char_offset = base_offset;

    for para in &paragraphs {
        if current.len() + para.len() > max_chars && !current.is_empty() {
            chunks.push(TextChunk {
                content: current.clone(),
                chunk_index: *chunk_index,
                section_title: title.clone(),
                char_offset,
            });
            *chunk_index += 1;

            if current.len() > overlap {
                let overlap_start = current.len() - overlap;
                current = current[overlap_start..].to_string();
                char_offset += overlap_start;
            } else {
                current.clear();
            }
        }

        // If a single paragraph exceeds max_chars, split by sentence boundaries
        if para.len() > max_chars {
            let sub_chunks = split_long_paragraph(para, title, char_offset, max_chars, overlap, chunk_index);
            chunks.extend(sub_chunks);
            char_offset += para.len();
            current.clear();
        } else {
            current.push_str(para);
            current.push_str("\n\n");
        }
    }

    if !current.trim().is_empty() {
        chunks.push(TextChunk {
            content: current.trim().to_string(),
            chunk_index: *chunk_index,
            section_title: title.clone(),
            char_offset,
        });
        *chunk_index += 1;
    }

    chunks
}

fn split_long_paragraph(
    para: &str,
    title: &Option<String>,
    base_offset: usize,
    max_chars: usize,
    overlap: usize,
    chunk_index: &mut usize,
) -> Vec<TextChunk> {
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < para.len() {
        let end = (start + max_chars).min(para.len());

        // Try to break at a sentence boundary (". ") within the last 20% of the chunk
        let break_at = if end < para.len() {
            let search_start = start + (max_chars * 4 / 5);
            para[search_start..end]
                .rfind(". ")
                .map(|pos| search_start + pos + 2)
                .unwrap_or(end)
        } else {
            end
        };

        chunks.push(TextChunk {
            content: para[start..break_at].trim().to_string(),
            chunk_index: *chunk_index,
            section_title: title.clone(),
            char_offset: base_offset + start,
        });
        *chunk_index += 1;

        if break_at >= para.len() {
            break;
        }

        start = if break_at > overlap {
            break_at - overlap
        } else {
            break_at
        };
    }

    chunks
}

fn merge_tiny_chunks(chunks: &mut Vec<TextChunk>, min_chars: usize) {
    let mut i = 0;
    while i < chunks.len() {
        if chunks[i].content.len() < min_chars && i + 1 < chunks.len() {
            let next = chunks.remove(i + 1);
            chunks[i].content.push_str("\n\n");
            chunks[i].content.push_str(&next.content);
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_by_headings() {
        let md = "## Medications\n\nMetformin 500mg twice daily for diabetes management prescribed by GP.\n\n## Lab Results\n\nHbA1c: 7.2% (elevated above target of 7.0%).\n\n## Instructions\n\nFollow up in 3 months for repeat blood work and medication review.";
        let chunker = MedicalChunker::new();
        let chunks = chunker.chunk(md);

        assert!(chunks.len() >= 3, "Expected >= 3 chunks, got {}", chunks.len());
        assert_eq!(chunks[0].section_title.as_deref(), Some("Medications"));
        assert_eq!(chunks[1].section_title.as_deref(), Some("Lab Results"));
        assert_eq!(chunks[2].section_title.as_deref(), Some("Instructions"));
    }

    #[test]
    fn splits_large_sections() {
        let large_section = "## Medications\n\n".to_string() + &"Medication details here. ".repeat(200);
        let chunker = MedicalChunker::new();
        let chunks = chunker.chunk(&large_section);

        assert!(chunks.len() > 1, "Large section should be split into multiple chunks");
        for chunk in &chunks {
            assert!(
                chunk.content.len() <= 1200,
                "Chunk too large: {} chars",
                chunk.content.len()
            );
        }
    }

    #[test]
    fn merges_tiny_sections() {
        let md = "## A\n\nShort.\n\n## B\n\nAlso ok but slightly longer content here to test merging of tiny sections with enough text.";
        let chunker = MedicalChunker::new();
        let chunks = chunker.chunk(md);

        for chunk in &chunks {
            assert!(
                chunk.content.len() >= 20,
                "Tiny chunk not merged: '{}'",
                chunk.content
            );
        }
    }

    #[test]
    fn preserves_chunk_indices() {
        let md = "## A\n\nSection A content is long enough to be a chunk.\n\n## B\n\nSection B also has enough content to be a chunk.";
        let chunker = MedicalChunker::new();
        let chunks = chunker.chunk(md);

        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.chunk_index, i, "Chunk index mismatch");
        }
    }

    #[test]
    fn empty_markdown_returns_empty() {
        let chunker = MedicalChunker::new();
        let chunks = chunker.chunk("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn no_headings_single_chunk() {
        let md = "This is a medical document without headings. It has enough text to be meaningful and should be treated as a single chunk.";
        let chunker = MedicalChunker::new();
        let chunks = chunker.chunk(md);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].section_title.is_none());
    }

    #[test]
    fn chunk_has_char_offset() {
        let md = "## First\n\nContent of first section with enough detail to exceed minimum.\n\n## Second\n\nContent of second section which is also long enough for chunking.";
        let chunker = MedicalChunker::new();
        let chunks = chunker.chunk(md);

        assert!(chunks.len() >= 2, "Expected >= 2 chunks, got {}", chunks.len());
        assert_eq!(chunks[0].char_offset, 0);
        assert!(chunks[1].char_offset > 0);
    }
}
