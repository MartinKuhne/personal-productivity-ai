---
title: Table Alignment Negative Test
tags:
- test
- table
---
# Table Vertical Alignment Test

The purpose of this test is to verify that all table cells are top-aligned, even when other cells in the same row contain wrapped, multi-line text that increases the row's height.

| Short Column (Left) | Center Column (Short) | Long Column (Right) |
|---|---|---|
| Alpha | Beta | This is a very long piece of text designed to wrap across multiple lines. When this text wraps, the height of the entire row will increase. The text in the "Alpha" and "Beta" cells must remain aligned to the top of their respective cells, rather than floating in the middle. |
| Gamma | Delta | Another long text block. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris. |
