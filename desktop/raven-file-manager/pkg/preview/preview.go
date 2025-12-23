package preview

import (
	"fmt"
	"image"
	_ "image/gif"
	_ "image/jpeg"
	_ "image/png"
	"os"
	"path/filepath"
	"strings"

	"raven-file-manager/pkg/fileview"

	"github.com/diamondburned/gotk4/pkg/gtk/v4"
)

// Type represents the type of preview to show
type Type int

const (
	TypeNone Type = iota
	TypeText
	TypeCode
	TypeImage
	TypeDirectory
	TypeBinary
)

// Panel manages the file preview
type Panel struct {
	ContentBox  *gtk.Box
	currentPath string
	Highlighter *SyntaxHighlighter
}

// NewPanel creates a new preview panel
func NewPanel() *Panel {
	return &Panel{
		Highlighter: NewSyntaxHighlighter(),
	}
}

// DetermineType determines the preview type for a file
func DetermineType(path string, entry fileview.FileEntry) Type {
	if entry.IsDir {
		return TypeDirectory
	}

	ext := strings.ToLower(filepath.Ext(path))

	imageExts := map[string]bool{
		".png": true, ".jpg": true, ".jpeg": true, ".gif": true,
		".bmp": true, ".webp": true, ".ico": true, ".svg": true,
	}
	if imageExts[ext] {
		return TypeImage
	}

	if fileview.IsCodeFile(path) {
		return TypeCode
	}

	if fileview.IsTextFile(path) {
		return TypeText
	}

	if fileview.IsBinaryFile(path) {
		return TypeBinary
	}

	return TypeText
}

// ShowPreview displays a preview for the given file
func (pp *Panel) ShowPreview(path string, entry fileview.FileEntry) {
	pp.currentPath = path
	pp.Clear()

	if pp.ContentBox == nil {
		return
	}

	previewType := DetermineType(path, entry)

	switch previewType {
	case TypeImage:
		pp.showImagePreview(path, entry)
	case TypeCode:
		pp.showCodePreview(path, entry)
	case TypeText:
		pp.showTextPreview(path, entry)
	case TypeDirectory:
		pp.showDirectoryPreview(path, entry)
	case TypeBinary:
		pp.showBinaryPreview(path, entry)
	default:
		pp.showNoPreview(entry)
	}
}

// Clear clears the current preview
func (pp *Panel) Clear() {
	if pp.ContentBox == nil {
		return
	}

	for {
		child := pp.ContentBox.FirstChild()
		if child == nil {
			break
		}
		pp.ContentBox.Remove(child)
	}
}

func (pp *Panel) showImagePreview(path string, entry fileview.FileEntry) {
	file, err := os.Open(path)
	if err != nil {
		pp.showError("Cannot load image: " + err.Error())
		return
	}
	defer file.Close()

	config, format, err := image.DecodeConfig(file)
	if err != nil {
		pp.showError("Cannot decode image")
		return
	}

	picture := gtk.NewPictureForFilename(path)
	picture.SetContentFit(gtk.ContentFitContain)
	picture.SetCanShrink(true)
	picture.AddCSSClass("preview-image")

	imageBox := gtk.NewBox(gtk.OrientationVertical, 8)
	imageBox.SetMarginStart(16)
	imageBox.SetMarginEnd(16)
	imageBox.SetMarginTop(16)
	imageBox.SetVExpand(true)
	imageBox.SetHExpand(true)
	imageBox.Append(picture)

	infoBox := gtk.NewBox(gtk.OrientationVertical, 4)
	infoBox.AddCSSClass("preview-info")

	dims := fmt.Sprintf("Dimensions: %d x %d", config.Width, config.Height)
	dimsLabel := gtk.NewLabel(dims)
	dimsLabel.AddCSSClass("preview-info-value")
	dimsLabel.SetHAlign(gtk.AlignStart)
	infoBox.Append(dimsLabel)

	formatLabel := gtk.NewLabel("Format: " + strings.ToUpper(format))
	formatLabel.AddCSSClass("preview-info-value")
	formatLabel.SetHAlign(gtk.AlignStart)
	infoBox.Append(formatLabel)

	sizeLabel := gtk.NewLabel("Size: " + fileview.HumanizeSize(entry.Size))
	sizeLabel.AddCSSClass("preview-info-value")
	sizeLabel.SetHAlign(gtk.AlignStart)
	infoBox.Append(sizeLabel)

	imageBox.Append(infoBox)

	pp.ContentBox.Append(imageBox)
}

func (pp *Panel) showCodePreview(path string, entry fileview.FileEntry) {
	content, err := pp.readFileContent(path, 1000)
	if err != nil {
		pp.showError("Cannot read file: " + err.Error())
		return
	}

	textView := gtk.NewTextView()
	textView.SetEditable(false)
	textView.SetCursorVisible(false)
	textView.SetWrapMode(gtk.WrapNone)
	textView.AddCSSClass("preview-text")
	textView.SetMonospace(true)

	buffer := textView.Buffer()
	buffer.SetText(content)

	scroll := gtk.NewScrolledWindow()
	scroll.SetPolicy(gtk.PolicyAutomatic, gtk.PolicyAutomatic)
	scroll.SetVExpand(true)
	scroll.SetChild(textView)

	pp.ContentBox.Append(scroll)
	pp.showFileInfo(entry)
}

func (pp *Panel) showTextPreview(path string, entry fileview.FileEntry) {
	content, err := pp.readFileContent(path, 500)
	if err != nil {
		pp.showError("Cannot read file: " + err.Error())
		return
	}

	textView := gtk.NewTextView()
	textView.SetEditable(false)
	textView.SetCursorVisible(false)
	textView.SetWrapMode(gtk.WrapWord)
	textView.AddCSSClass("preview-text")

	buffer := textView.Buffer()
	buffer.SetText(content)

	scroll := gtk.NewScrolledWindow()
	scroll.SetPolicy(gtk.PolicyAutomatic, gtk.PolicyAutomatic)
	scroll.SetVExpand(true)
	scroll.SetChild(textView)

	pp.ContentBox.Append(scroll)
	pp.showFileInfo(entry)
}

func (pp *Panel) showDirectoryPreview(path string, entry fileview.FileEntry) {
	var fileCount, dirCount int
	var totalSize int64

	entries, err := os.ReadDir(path)
	if err == nil {
		for _, e := range entries {
			if e.IsDir() {
				dirCount++
			} else {
				fileCount++
				info, err := e.Info()
				if err == nil {
					totalSize += info.Size()
				}
			}
		}
	}

	infoBox := gtk.NewBox(gtk.OrientationVertical, 8)
	infoBox.AddCSSClass("preview-info")
	infoBox.SetMarginStart(16)
	infoBox.SetMarginEnd(16)
	infoBox.SetMarginTop(16)

	icon := gtk.NewImageFromIconName("folder-symbolic")
	icon.SetPixelSize(64)
	icon.AddCSSClass("file-icon-folder")
	icon.SetMarginBottom(16)
	infoBox.Append(icon)

	contentsLabel := gtk.NewLabel("Contents")
	contentsLabel.AddCSSClass("preview-title")
	contentsLabel.SetHAlign(gtk.AlignStart)
	contentsLabel.SetMarginBottom(8)
	infoBox.Append(contentsLabel)

	grid := gtk.NewGrid()
	grid.SetColumnSpacing(16)
	grid.SetRowSpacing(4)

	row := 0

	if dirCount > 0 {
		folderLabel := gtk.NewLabel("Folders:")
		folderLabel.AddCSSClass("preview-info-label")
		folderLabel.SetHAlign(gtk.AlignEnd)
		grid.Attach(folderLabel, 0, row, 1, 1)

		folderValue := gtk.NewLabel(fmt.Sprintf("%d", dirCount))
		folderValue.AddCSSClass("preview-info-value")
		folderValue.SetHAlign(gtk.AlignStart)
		grid.Attach(folderValue, 1, row, 1, 1)
		row++
	}

	if fileCount > 0 {
		fileLabel := gtk.NewLabel("Files:")
		fileLabel.AddCSSClass("preview-info-label")
		fileLabel.SetHAlign(gtk.AlignEnd)
		grid.Attach(fileLabel, 0, row, 1, 1)

		fileValue := gtk.NewLabel(fmt.Sprintf("%d", fileCount))
		fileValue.AddCSSClass("preview-info-value")
		fileValue.SetHAlign(gtk.AlignStart)
		grid.Attach(fileValue, 1, row, 1, 1)
		row++
	}

	sizeLabel := gtk.NewLabel("Total Size:")
	sizeLabel.AddCSSClass("preview-info-label")
	sizeLabel.SetHAlign(gtk.AlignEnd)
	grid.Attach(sizeLabel, 0, row, 1, 1)

	sizeValue := gtk.NewLabel(fileview.HumanizeSize(totalSize))
	sizeValue.AddCSSClass("preview-info-value")
	sizeValue.SetHAlign(gtk.AlignStart)
	grid.Attach(sizeValue, 1, row, 1, 1)
	row++

	modLabel := gtk.NewLabel("Modified:")
	modLabel.AddCSSClass("preview-info-label")
	modLabel.SetHAlign(gtk.AlignEnd)
	grid.Attach(modLabel, 0, row, 1, 1)

	modValue := gtk.NewLabel(entry.ModTime.Format("Jan 2, 2006 3:04 PM"))
	modValue.AddCSSClass("preview-info-value")
	modValue.SetHAlign(gtk.AlignStart)
	grid.Attach(modValue, 1, row, 1, 1)

	infoBox.Append(grid)

	pp.ContentBox.Append(infoBox)
}

func (pp *Panel) showBinaryPreview(path string, entry fileview.FileEntry) {
	infoBox := gtk.NewBox(gtk.OrientationVertical, 8)
	infoBox.AddCSSClass("preview-info")
	infoBox.SetMarginStart(16)
	infoBox.SetMarginEnd(16)
	infoBox.SetMarginTop(16)

	icon := gtk.NewImageFromIconName(fileview.GetFileIcon(entry))
	icon.SetPixelSize(64)
	icon.SetMarginBottom(16)
	infoBox.Append(icon)

	msgLabel := gtk.NewLabel("Binary file - preview not available")
	msgLabel.AddCSSClass("status-text")
	infoBox.Append(msgLabel)

	pp.showFileInfoIn(entry, infoBox)

	pp.ContentBox.Append(infoBox)
}

func (pp *Panel) showNoPreview(entry fileview.FileEntry) {
	infoBox := gtk.NewBox(gtk.OrientationVertical, 8)
	infoBox.AddCSSClass("preview-info")
	infoBox.SetMarginStart(16)
	infoBox.SetMarginEnd(16)
	infoBox.SetMarginTop(16)

	icon := gtk.NewImageFromIconName(fileview.GetFileIcon(entry))
	icon.SetPixelSize(64)
	icon.SetMarginBottom(16)
	infoBox.Append(icon)

	msgLabel := gtk.NewLabel("Preview not available")
	msgLabel.AddCSSClass("status-text")
	infoBox.Append(msgLabel)

	pp.showFileInfoIn(entry, infoBox)

	pp.ContentBox.Append(infoBox)
}

func (pp *Panel) showError(message string) {
	if pp.ContentBox == nil {
		return
	}

	errorBox := gtk.NewBox(gtk.OrientationVertical, 8)
	errorBox.SetMarginStart(16)
	errorBox.SetMarginEnd(16)
	errorBox.SetMarginTop(16)

	icon := gtk.NewImageFromIconName("dialog-error-symbolic")
	icon.SetPixelSize(48)
	errorBox.Append(icon)

	label := gtk.NewLabel(message)
	label.AddCSSClass("status-text")
	label.SetWrap(true)
	errorBox.Append(label)

	pp.ContentBox.Append(errorBox)
}

func (pp *Panel) showFileInfo(entry fileview.FileEntry) {
	if pp.ContentBox == nil {
		return
	}

	infoBox := gtk.NewBox(gtk.OrientationVertical, 4)
	infoBox.AddCSSClass("preview-info")
	infoBox.SetMarginStart(16)
	infoBox.SetMarginEnd(16)
	infoBox.SetMarginTop(8)

	pp.showFileInfoIn(entry, infoBox)

	pp.ContentBox.Append(infoBox)
}

func (pp *Panel) showFileInfoIn(entry fileview.FileEntry, box *gtk.Box) {
	grid := gtk.NewGrid()
	grid.SetColumnSpacing(12)
	grid.SetRowSpacing(4)

	row := 0

	sizeLabel := gtk.NewLabel("Size:")
	sizeLabel.AddCSSClass("preview-info-label")
	sizeLabel.SetHAlign(gtk.AlignEnd)
	grid.Attach(sizeLabel, 0, row, 1, 1)

	sizeValue := gtk.NewLabel(fileview.HumanizeSize(entry.Size))
	sizeValue.AddCSSClass("preview-info-value")
	sizeValue.SetHAlign(gtk.AlignStart)
	grid.Attach(sizeValue, 1, row, 1, 1)
	row++

	typeLabel := gtk.NewLabel("Type:")
	typeLabel.AddCSSClass("preview-info-label")
	typeLabel.SetHAlign(gtk.AlignEnd)
	grid.Attach(typeLabel, 0, row, 1, 1)

	typeValue := gtk.NewLabel(fileview.GetFileTypeDescription(entry))
	typeValue.AddCSSClass("preview-info-value")
	typeValue.SetHAlign(gtk.AlignStart)
	grid.Attach(typeValue, 1, row, 1, 1)
	row++

	modLabel := gtk.NewLabel("Modified:")
	modLabel.AddCSSClass("preview-info-label")
	modLabel.SetHAlign(gtk.AlignEnd)
	grid.Attach(modLabel, 0, row, 1, 1)

	modValue := gtk.NewLabel(entry.ModTime.Format("Jan 2, 2006 3:04 PM"))
	modValue.AddCSSClass("preview-info-value")
	modValue.SetHAlign(gtk.AlignStart)
	grid.Attach(modValue, 1, row, 1, 1)

	box.Append(grid)
}

func (pp *Panel) readFileContent(path string, maxLines int) (string, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer file.Close()

	info, _ := file.Stat()
	maxBytes := int64(maxLines * 200)
	if info.Size() < maxBytes {
		maxBytes = info.Size()
	}

	content := make([]byte, maxBytes)
	n, err := file.Read(content)
	if err != nil {
		return "", err
	}

	return string(content[:n]), nil
}
