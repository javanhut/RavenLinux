package search

import (
	"bufio"
	"context"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"sync"
	"unicode"

	"raven-file-manager/pkg/fileview"
)

// Engine handles fuzzy file search and content search
type Engine struct {
	mu         sync.RWMutex
	searching  bool
	cancelFunc context.CancelFunc
}

// Result represents a search match
type Result struct {
	Entry     fileview.FileEntry
	Score     int
	Indices   []int
	MatchType string
	Context   string
	LineNum   int
}

// FuzzyMatcher implements fzf-style fuzzy matching
type FuzzyMatcher struct {
	pattern       string
	lowerPattern  string
	caseSensitive bool
}

// Scoring constants
const (
	ScoreMatch        = 16
	ScoreGapStart     = -3
	ScoreGapExtension = -1
	BonusBoundary     = 8
	BonusConsecutive  = 4
	BonusFirstChar    = 4
	BonusCamelCase    = 2
)

// NewEngine creates a new search engine
func NewEngine() *Engine {
	return &Engine{}
}

// NewFuzzyMatcher creates a new fuzzy matcher
func NewFuzzyMatcher(pattern string) *FuzzyMatcher {
	caseSensitive := false
	for _, r := range pattern {
		if unicode.IsUpper(r) {
			caseSensitive = true
			break
		}
	}

	return &FuzzyMatcher{
		pattern:       pattern,
		lowerPattern:  strings.ToLower(pattern),
		caseSensitive: caseSensitive,
	}
}

// Match performs fuzzy matching and returns score and match positions
func (m *FuzzyMatcher) Match(text string) (int, []int, bool) {
	if m.pattern == "" {
		return 0, nil, true
	}

	var compareText string
	if m.caseSensitive {
		compareText = text
	} else {
		compareText = strings.ToLower(text)
	}

	pattern := m.lowerPattern
	if m.caseSensitive {
		pattern = m.pattern
	}

	return m.findBestMatch(text, compareText, pattern)
}

func (m *FuzzyMatcher) findBestMatch(original, text, pattern string) (int, []int, bool) {
	patternLen := len(pattern)
	textLen := len(text)

	if patternLen == 0 {
		return 0, nil, true
	}

	if patternLen > textLen {
		return 0, nil, false
	}

	indices := make([]int, 0, patternLen)
	score := 0
	pi := 0
	prevMatched := false
	prevBoundary := true
	consecutiveBonus := 0

	for ti := 0; ti < textLen && pi < patternLen; ti++ {
		pc := pattern[pi]
		tc := text[ti]
		origChar := original[ti]

		if tc == pc {
			indices = append(indices, ti)
			score += ScoreMatch

			if prevMatched {
				consecutiveBonus += BonusConsecutive
				score += consecutiveBonus
			} else {
				consecutiveBonus = 0
				if len(indices) > 1 {
					gap := ti - indices[len(indices)-2] - 1
					if gap > 0 {
						score += ScoreGapStart + (gap-1)*ScoreGapExtension
					}
				}
			}

			if ti == 0 {
				score += BonusFirstChar
			}

			if prevBoundary {
				score += BonusBoundary
			}

			if ti > 0 && unicode.IsLower(rune(original[ti-1])) && unicode.IsUpper(rune(origChar)) {
				score += BonusCamelCase
			}

			prevMatched = true
			pi++
		} else {
			prevMatched = false
		}

		prevBoundary = isBoundary(origChar)
	}

	if pi < patternLen {
		return 0, nil, false
	}

	score += (100 - textLen) / 10

	return score, indices, true
}

func isBoundary(c byte) bool {
	return c == '/' || c == '.' || c == '_' || c == '-' || c == ' '
}

// Search performs a search in the directory tree
func (se *Engine) Search(ctx context.Context, root, query string, maxResults int) []Result {
	se.mu.Lock()
	if se.searching && se.cancelFunc != nil {
		se.cancelFunc()
	}
	ctx, se.cancelFunc = context.WithCancel(ctx)
	se.searching = true
	se.mu.Unlock()

	defer func() {
		se.mu.Lock()
		se.searching = false
		se.mu.Unlock()
	}()

	parsed := parseSearchQuery(query)
	results := make([]Result, 0, maxResults)
	matcher := NewFuzzyMatcher(parsed.pattern)

	filepath.WalkDir(root, func(path string, d os.DirEntry, err error) error {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		if err != nil {
			return nil
		}

		if d.IsDir() && strings.HasPrefix(d.Name(), ".") && d.Name() != "." {
			return filepath.SkipDir
		}

		relPath, _ := filepath.Rel(root, path)
		if relPath == "." {
			return nil
		}

		score, indices, matched := matcher.Match(d.Name())
		if !matched {
			score, indices, matched = matcher.Match(relPath)
		}

		if matched && score > 0 {
			info, err := d.Info()
			if err != nil {
				return nil
			}

			entry := fileview.FileEntry{
				Name:     d.Name(),
				Path:     path,
				Size:     info.Size(),
				ModTime:  info.ModTime(),
				Mode:     info.Mode(),
				IsDir:    d.IsDir(),
				IsHidden: strings.HasPrefix(d.Name(), "."),
			}

			if parsed.exclude != "" {
				excludeMatcher := NewFuzzyMatcher(parsed.exclude)
				_, _, excludeMatched := excludeMatcher.Match(d.Name())
				if excludeMatched {
					return nil
				}
			}

			results = append(results, Result{
				Entry:     entry,
				Score:     score,
				Indices:   indices,
				MatchType: "filename",
			})
		}

		if len(results) >= maxResults {
			return filepath.SkipAll
		}

		return nil
	})

	sort.Slice(results, func(i, j int) bool {
		return results[i].Score > results[j].Score
	})

	return results
}

// ContentSearch searches file contents for a pattern
func (se *Engine) ContentSearch(ctx context.Context, root, pattern string, maxFileSize int64, maxResults int) []Result {
	se.mu.Lock()
	if se.searching && se.cancelFunc != nil {
		se.cancelFunc()
	}
	ctx, se.cancelFunc = context.WithCancel(ctx)
	se.searching = true
	se.mu.Unlock()

	defer func() {
		se.mu.Lock()
		se.searching = false
		se.mu.Unlock()
	}()

	results := make([]Result, 0)
	resultMu := sync.Mutex{}

	re, err := regexp.Compile("(?i)" + regexp.QuoteMeta(pattern))
	if err != nil {
		return results
	}

	fileChan := make(chan string, 100)
	var wg sync.WaitGroup

	numWorkers := 4
	for i := 0; i < numWorkers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for path := range fileChan {
				select {
				case <-ctx.Done():
					return
				default:
				}

				matches := searchFileContent(path, re, pattern)
				if len(matches) > 0 {
					resultMu.Lock()
					results = append(results, matches...)
					resultMu.Unlock()
				}
			}
		}()
	}

	filepath.WalkDir(root, func(path string, d os.DirEntry, err error) error {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		if err != nil || d.IsDir() {
			return nil
		}

		if strings.HasPrefix(d.Name(), ".") {
			return nil
		}

		if fileview.IsBinaryFile(path) {
			return nil
		}

		info, err := d.Info()
		if err != nil || info.Size() > maxFileSize {
			return nil
		}

		resultMu.Lock()
		count := len(results)
		resultMu.Unlock()
		if count >= maxResults {
			return filepath.SkipAll
		}

		fileChan <- path
		return nil
	})

	close(fileChan)
	wg.Wait()

	sort.Slice(results, func(i, j int) bool {
		return results[i].Score > results[j].Score
	})

	if len(results) > maxResults {
		results = results[:maxResults]
	}

	return results
}

func searchFileContent(path string, re *regexp.Regexp, pattern string) []Result {
	file, err := os.Open(path)
	if err != nil {
		return nil
	}
	defer file.Close()

	info, _ := file.Stat()

	var results []Result
	scanner := bufio.NewScanner(file)
	lineNum := 0

	for scanner.Scan() {
		lineNum++
		line := scanner.Text()

		if re.MatchString(line) {
			entry := fileview.FileEntry{
				Name:    filepath.Base(path),
				Path:    path,
				Size:    info.Size(),
				ModTime: info.ModTime(),
				IsDir:   false,
			}

			indices := re.FindStringIndex(line)

			results = append(results, Result{
				Entry:     entry,
				Score:     100 - lineNum,
				Indices:   []int{indices[0], indices[1]},
				MatchType: "content",
				Context:   truncateLine(line, 100),
				LineNum:   lineNum,
			})

			if len(results) >= 5 {
				break
			}
		}
	}

	return results
}

func truncateLine(line string, maxLen int) string {
	if len(line) <= maxLen {
		return line
	}
	return line[:maxLen-3] + "..."
}

type parsedQuery struct {
	pattern string
	exclude string
	terms   []string
}

func parseSearchQuery(query string) parsedQuery {
	parsed := parsedQuery{}

	if strings.HasPrefix(query, "!") {
		parsed.exclude = strings.TrimPrefix(query, "!")
		return parsed
	}

	if idx := strings.Index(query, " !"); idx != -1 {
		parsed.pattern = strings.TrimSpace(query[:idx])
		parsed.exclude = strings.TrimSpace(query[idx+2:])
		return parsed
	}

	if strings.Contains(query, "|") {
		parts := strings.Split(query, "|")
		for _, p := range parts {
			parsed.terms = append(parsed.terms, strings.TrimSpace(p))
		}
		parsed.pattern = parsed.terms[0]
		return parsed
	}

	if strings.Contains(query, " ") {
		parsed.terms = strings.Fields(query)
		parsed.pattern = query
	} else {
		parsed.pattern = query
	}

	return parsed
}

// Cancel cancels any ongoing search
func (se *Engine) Cancel() {
	se.mu.Lock()
	defer se.mu.Unlock()

	if se.cancelFunc != nil {
		se.cancelFunc()
	}
	se.searching = false
}

// IsSearching returns true if a search is in progress
func (se *Engine) IsSearching() bool {
	se.mu.RLock()
	defer se.mu.RUnlock()
	return se.searching
}
