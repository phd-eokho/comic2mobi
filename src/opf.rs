//! The kindlegen source documents of a fixed-layout comic: one OPF package,
//! one NCX table of contents and one HTML page per image.

use crate::source::Page;

/// Page box every image is fitted into; 3:4 like a Kindle screen.
const PAGE_WIDTH: u32 = 768;
const PAGE_HEIGHT: u32 = 1024;

pub const OPF_NAME: &str = "book.opf";
pub const NCX_NAME: &str = "toc.ncx";

pub fn page_name(index: usize) -> String {
    format!("{index:05}.html")
}

pub fn opf(title: &str, author: &str, pages: &[Page]) -> String {
    let (title, author) = (escape(title), escape(author));
    let manifest: String = pages
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let (html, image, media_type) =
                (page_name(index), &page.file, page.image.format.media_type());
            format!(
                r#"    <item id="page_{index:05}" href="{html}" media-type="application/xhtml+xml"/>
    <item id="img_{index:05}" href="{image}" media-type="{media_type}"/>
"#
            )
        })
        .collect();
    let spine: String = (0..pages.len())
        .map(|index| format!("    <itemref idref=\"page_{index:05}\"/>\n"))
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<package version="2.0" unique-identifier="PrimaryID" xmlns="http://www.idpf.org/2007/opf">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>{title}</dc:title>
    <dc:language>en</dc:language>
    <dc:identifier id="PrimaryID">{title}</dc:identifier>
    <dc:creator opf:role="aut">{author}</dc:creator>
    <meta name="cover" content="img_00000"/>
    <meta name="book-type" content="comic"/>
    <meta name="zero-gutter" content="true"/>
    <meta name="zero-margin" content="true"/>
    <meta name="RegionMagnification" content="true"/>
    <meta name="fixed-layout" content="true"/>
    <meta name="orientation-lock" content="portrait"/>
    <meta name="original-resolution" content="{PAGE_WIDTH}x{PAGE_HEIGHT}"/>
  </metadata>
  <manifest>
    <item id="ncx" href="{NCX_NAME}" media-type="application/x-dtbncx+xml"/>
{manifest}  </manifest>
  <spine toc="ncx">
{spine}  </spine>
</package>
"#
    )
}

pub fn ncx(title: &str, author: &str, pages: &[Page]) -> String {
    let (title, author) = (escape(title), escape(author));
    let nav_map: String = pages
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let (order, label, html) = (index + 1, escape(&page.label), page_name(index));
            format!(
                r#"    <navPoint class="other" id="nav_{index:05}" playOrder="{order}"><navLabel><text>{label}</text></navLabel><content src="{html}"/></navPoint>
"#
            )
        })
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd">
<ncx version="2005-1" xml:lang="en-US" xmlns="http://www.daisy.org/z3986/2005/ncx/">
  <head>
    <meta name="dtb:uid" content="{title}"/>
    <meta name="dtb:depth" content="1"/>
    <meta name="dtb:totalPageCount" content="0"/>
    <meta name="dtb:maxPageNumber" content="0"/>
  </head>
  <docTitle><text>{title}</text></docTitle>
  <docAuthor><text>{author}</text></docAuthor>
  <navMap>
{nav_map}  </navMap>
</ncx>
"#
    )
}

/// One page: the image scaled to fit the page box and centred in it.
pub fn page_html(page: &Page) -> String {
    let (image_width, image_height) = (u64::from(page.image.width), u64::from(page.image.height));
    let (box_width, box_height) = (u64::from(PAGE_WIDTH), u64::from(PAGE_HEIGHT));
    let (width, height) = if image_width * box_height > image_height * box_width {
        (box_width, (image_height * box_width / image_width).max(1))
    } else {
        ((image_width * box_height / image_height).max(1), box_height)
    };
    let (left, top) = ((box_width - width) / 2, (box_height - height) / 2);
    let (label, file) = (escape(&page.label), &page.file);
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
<meta http-equiv="Content-Type" content="text/html; charset=utf-8"/>
<title>{label}</title>
<style type="text/css">
html,body{{margin:0;padding:0;background:#FFF;}}
div.fs{{position:relative;width:{PAGE_WIDTH}px;height:{PAGE_HEIGHT}px;}}
img.singlePage{{position:absolute;left:{left}px;top:{top}px;width:{width}px;height:{height}px;}}
</style>
</head>
<body><div class="fs"><img class="singlePage" src="{file}" alt=""/></div></body>
</html>
"#
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
