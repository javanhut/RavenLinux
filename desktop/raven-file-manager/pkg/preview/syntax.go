package preview

import (
	"path/filepath"
	"regexp"
	"strings"
)

// TokenType represents a syntax token type
type TokenType int

const (
	TokenNone TokenType = iota
	TokenKeyword
	TokenString
	TokenComment
	TokenNumber
	TokenFunction
	TokenTypeName
	TokenOperator
	TokenVariable
)

// HighlightRegion represents a highlighted region in the text
type HighlightRegion struct {
	Start     int
	End       int
	TokenType TokenType
}

// SyntaxHighlighter handles syntax highlighting for code files
type SyntaxHighlighter struct {
	languages map[string]*LanguageDefinition
}

// LanguageDefinition defines syntax rules for a language
type LanguageDefinition struct {
	Name              string
	Keywords          []string
	Types             []string
	StringDelims      []string
	CommentLine       string
	CommentBlockStart string
	CommentBlockEnd   string
	NumberPattern     *regexp.Regexp
	FunctionPattern   *regexp.Regexp
}

// NewSyntaxHighlighter creates a new syntax highlighter
func NewSyntaxHighlighter() *SyntaxHighlighter {
	sh := &SyntaxHighlighter{
		languages: make(map[string]*LanguageDefinition),
	}

	sh.registerLanguages()
	return sh
}

func (sh *SyntaxHighlighter) registerLanguages() {
	sh.languages["go"] = &LanguageDefinition{
		Name: "Go",
		Keywords: []string{
			"break", "case", "chan", "const", "continue", "default", "defer",
			"else", "fallthrough", "for", "func", "go", "goto", "if", "import",
			"interface", "map", "package", "range", "return", "select", "struct",
			"switch", "type", "var",
		},
		Types: []string{
			"bool", "byte", "complex64", "complex128", "error", "float32",
			"float64", "int", "int8", "int16", "int32", "int64", "rune",
			"string", "uint", "uint8", "uint16", "uint32", "uint64", "uintptr",
			"true", "false", "nil",
		},
		StringDelims:      []string{"\"", "`"},
		CommentLine:       "//",
		CommentBlockStart: "/*",
		CommentBlockEnd:   "*/",
		NumberPattern:     regexp.MustCompile(`\b\d+(\.\d+)?\b`),
		FunctionPattern:   regexp.MustCompile(`\b([a-zA-Z_][a-zA-Z0-9_]*)\s*\(`),
	}

	sh.languages["python"] = &LanguageDefinition{
		Name: "Python",
		Keywords: []string{
			"and", "as", "assert", "async", "await", "break", "class", "continue",
			"def", "del", "elif", "else", "except", "finally", "for", "from",
			"global", "if", "import", "in", "is", "lambda", "nonlocal", "not",
			"or", "pass", "raise", "return", "try", "while", "with", "yield",
		},
		Types: []string{
			"True", "False", "None", "int", "float", "str", "list", "dict",
			"tuple", "set", "bool", "bytes",
		},
		StringDelims:    []string{"\"", "'", "\"\"\"", "'''"},
		CommentLine:     "#",
		NumberPattern:   regexp.MustCompile(`\b\d+(\.\d+)?\b`),
		FunctionPattern: regexp.MustCompile(`\bdef\s+([a-zA-Z_][a-zA-Z0-9_]*)`),
	}

	sh.languages["javascript"] = &LanguageDefinition{
		Name: "JavaScript",
		Keywords: []string{
			"async", "await", "break", "case", "catch", "class", "const",
			"continue", "debugger", "default", "delete", "do", "else", "export",
			"extends", "finally", "for", "function", "if", "import", "in",
			"instanceof", "let", "new", "return", "static", "super", "switch",
			"this", "throw", "try", "typeof", "var", "void", "while", "with",
			"yield",
		},
		Types: []string{
			"true", "false", "null", "undefined", "NaN", "Infinity",
		},
		StringDelims:      []string{"\"", "'", "`"},
		CommentLine:       "//",
		CommentBlockStart: "/*",
		CommentBlockEnd:   "*/",
		NumberPattern:     regexp.MustCompile(`\b\d+(\.\d+)?\b`),
		FunctionPattern:   regexp.MustCompile(`\bfunction\s+([a-zA-Z_][a-zA-Z0-9_]*)`),
	}
	sh.languages["typescript"] = sh.languages["javascript"]

	sh.languages["rust"] = &LanguageDefinition{
		Name: "Rust",
		Keywords: []string{
			"as", "async", "await", "break", "const", "continue", "crate",
			"dyn", "else", "enum", "extern", "false", "fn", "for", "if",
			"impl", "in", "let", "loop", "match", "mod", "move", "mut",
			"pub", "ref", "return", "self", "Self", "static", "struct",
			"super", "trait", "true", "type", "unsafe", "use", "where", "while",
		},
		Types: []string{
			"bool", "char", "str", "i8", "i16", "i32", "i64", "i128", "isize",
			"u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64",
			"String", "Vec", "Option", "Result", "Box", "Rc", "Arc",
		},
		StringDelims:      []string{"\""},
		CommentLine:       "//",
		CommentBlockStart: "/*",
		CommentBlockEnd:   "*/",
		NumberPattern:     regexp.MustCompile(`\b\d+(\.\d+)?\b`),
		FunctionPattern:   regexp.MustCompile(`\bfn\s+([a-zA-Z_][a-zA-Z0-9_]*)`),
	}

	sh.languages["c"] = &LanguageDefinition{
		Name: "C",
		Keywords: []string{
			"auto", "break", "case", "const", "continue", "default", "do",
			"else", "enum", "extern", "for", "goto", "if", "inline", "register",
			"restrict", "return", "sizeof", "static", "struct", "switch",
			"typedef", "union", "volatile", "while",
		},
		Types: []string{
			"char", "double", "float", "int", "long", "short", "signed",
			"unsigned", "void", "NULL", "true", "false",
		},
		StringDelims:      []string{"\"", "'"},
		CommentLine:       "//",
		CommentBlockStart: "/*",
		CommentBlockEnd:   "*/",
		NumberPattern:     regexp.MustCompile(`\b\d+(\.\d+)?\b`),
		FunctionPattern:   regexp.MustCompile(`\b([a-zA-Z_][a-zA-Z0-9_]*)\s*\(`),
	}
	sh.languages["cpp"] = sh.languages["c"]

	sh.languages["shell"] = &LanguageDefinition{
		Name: "Shell",
		Keywords: []string{
			"if", "then", "else", "elif", "fi", "case", "esac", "for", "while",
			"do", "done", "in", "function", "select", "until", "return", "exit",
			"break", "continue", "local", "export", "readonly", "declare",
		},
		Types:           []string{"true", "false"},
		StringDelims:    []string{"\"", "'"},
		CommentLine:     "#",
		NumberPattern:   regexp.MustCompile(`\b\d+\b`),
		FunctionPattern: regexp.MustCompile(`\bfunction\s+([a-zA-Z_][a-zA-Z0-9_]*)`),
	}
	sh.languages["bash"] = sh.languages["shell"]
	sh.languages["sh"] = sh.languages["shell"]

	sh.languages["json"] = &LanguageDefinition{
		Name:          "JSON",
		Keywords:      []string{},
		Types:         []string{"true", "false", "null"},
		StringDelims:  []string{"\""},
		NumberPattern: regexp.MustCompile(`\b-?\d+(\.\d+)?([eE][+-]?\d+)?\b`),
	}

	sh.languages["yaml"] = &LanguageDefinition{
		Name:          "YAML",
		Keywords:      []string{},
		Types:         []string{"true", "false", "null", "yes", "no", "on", "off"},
		StringDelims:  []string{"\"", "'"},
		CommentLine:   "#",
		NumberPattern: regexp.MustCompile(`\b-?\d+(\.\d+)?\b`),
	}
	sh.languages["yml"] = sh.languages["yaml"]
}

// GetLanguage returns the language definition for a file
func (sh *SyntaxHighlighter) GetLanguage(path string) *LanguageDefinition {
	ext := strings.ToLower(filepath.Ext(path))
	if ext == "" {
		return nil
	}

	ext = ext[1:]

	extMap := map[string]string{
		"go":   "go",
		"py":   "python",
		"js":   "javascript",
		"jsx":  "javascript",
		"ts":   "typescript",
		"tsx":  "typescript",
		"rs":   "rust",
		"c":    "c",
		"cpp":  "cpp",
		"cc":   "cpp",
		"cxx":  "cpp",
		"h":    "c",
		"hpp":  "cpp",
		"sh":   "shell",
		"bash": "shell",
		"zsh":  "shell",
		"json": "json",
		"yaml": "yaml",
		"yml":  "yaml",
	}

	if lang, ok := extMap[ext]; ok {
		return sh.languages[lang]
	}

	return nil
}

// Highlight applies syntax highlighting to the given code
func (sh *SyntaxHighlighter) Highlight(code, path string) []HighlightRegion {
	lang := sh.GetLanguage(path)
	if lang == nil {
		return nil
	}

	regions := make([]HighlightRegion, 0)

	regions = append(regions, sh.findComments(code, lang)...)
	regions = append(regions, sh.findStrings(code, lang)...)

	if lang.NumberPattern != nil {
		matches := lang.NumberPattern.FindAllStringIndex(code, -1)
		for _, m := range matches {
			if !sh.isInRegion(m[0], regions) {
				regions = append(regions, HighlightRegion{
					Start:     m[0],
					End:       m[1],
					TokenType: TokenNumber,
				})
			}
		}
	}

	for _, kw := range lang.Keywords {
		pattern := regexp.MustCompile(`\b` + regexp.QuoteMeta(kw) + `\b`)
		matches := pattern.FindAllStringIndex(code, -1)
		for _, m := range matches {
			if !sh.isInRegion(m[0], regions) {
				regions = append(regions, HighlightRegion{
					Start:     m[0],
					End:       m[1],
					TokenType: TokenKeyword,
				})
			}
		}
	}

	for _, t := range lang.Types {
		pattern := regexp.MustCompile(`\b` + regexp.QuoteMeta(t) + `\b`)
		matches := pattern.FindAllStringIndex(code, -1)
		for _, m := range matches {
			if !sh.isInRegion(m[0], regions) {
				regions = append(regions, HighlightRegion{
					Start:     m[0],
					End:       m[1],
					TokenType: TokenTypeName,
				})
			}
		}
	}

	if lang.FunctionPattern != nil {
		matches := lang.FunctionPattern.FindAllStringSubmatchIndex(code, -1)
		for _, m := range matches {
			if len(m) >= 4 && !sh.isInRegion(m[2], regions) {
				regions = append(regions, HighlightRegion{
					Start:     m[2],
					End:       m[3],
					TokenType: TokenFunction,
				})
			}
		}
	}

	return regions
}

func (sh *SyntaxHighlighter) findComments(code string, lang *LanguageDefinition) []HighlightRegion {
	regions := make([]HighlightRegion, 0)

	if lang.CommentLine != "" {
		lines := strings.Split(code, "\n")
		pos := 0
		for _, line := range lines {
			idx := strings.Index(line, lang.CommentLine)
			if idx != -1 {
				regions = append(regions, HighlightRegion{
					Start:     pos + idx,
					End:       pos + len(line),
					TokenType: TokenComment,
				})
			}
			pos += len(line) + 1
		}
	}

	if lang.CommentBlockStart != "" && lang.CommentBlockEnd != "" {
		pos := 0
		for {
			start := strings.Index(code[pos:], lang.CommentBlockStart)
			if start == -1 {
				break
			}
			start += pos

			end := strings.Index(code[start+len(lang.CommentBlockStart):], lang.CommentBlockEnd)
			if end == -1 {
				regions = append(regions, HighlightRegion{
					Start:     start,
					End:       len(code),
					TokenType: TokenComment,
				})
				break
			}
			end += start + len(lang.CommentBlockStart) + len(lang.CommentBlockEnd)

			regions = append(regions, HighlightRegion{
				Start:     start,
				End:       end,
				TokenType: TokenComment,
			})

			pos = end
		}
	}

	return regions
}

func (sh *SyntaxHighlighter) findStrings(code string, lang *LanguageDefinition) []HighlightRegion {
	regions := make([]HighlightRegion, 0)

	for _, delim := range lang.StringDelims {
		pos := 0
		for {
			start := strings.Index(code[pos:], delim)
			if start == -1 {
				break
			}
			start += pos

			if start > 0 && code[start-1] == '\\' {
				pos = start + 1
				continue
			}

			if sh.isInRegion(start, regions) {
				pos = start + 1
				continue
			}

			end := start + len(delim)
			for end < len(code) {
				idx := strings.Index(code[end:], delim)
				if idx == -1 {
					end = len(code)
					break
				}
				end += idx

				escapeCount := 0
				for i := end - 1; i >= start+len(delim) && code[i] == '\\'; i-- {
					escapeCount++
				}

				if escapeCount%2 == 0 {
					end += len(delim)
					break
				}

				end++
			}

			regions = append(regions, HighlightRegion{
				Start:     start,
				End:       end,
				TokenType: TokenString,
			})

			pos = end
		}
	}

	return regions
}

func (sh *SyntaxHighlighter) isInRegion(pos int, regions []HighlightRegion) bool {
	for _, r := range regions {
		if pos >= r.Start && pos < r.End {
			return true
		}
	}
	return false
}
