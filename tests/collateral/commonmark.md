# CommonMark Spec Examples

## Tabs

[CM-001] Example 1
**Input:**
```
→foo→baz→→bim
```
**Output:**
```html
<pre><code>foo→baz→→bim
</code></pre>
```

## Tabs

[CM-002] Example 2
**Input:**
```
  →foo→baz→→bim
```
**Output:**
```html
<pre><code>foo→baz→→bim
</code></pre>
```

## Tabs

[CM-003] Example 3
**Input:**
```
    a→a
    ὐ→a
```
**Output:**
```html
<pre><code>a→a
ὐ→a
</code></pre>
```

## Tabs

[CM-004] Example 4
**Input:**
```
  - foo

→bar
```
**Output:**
```html
<ul>
<li>
<p>foo</p>
<p>bar</p>
</li>
</ul>
```

## Tabs

[CM-005] Example 5
**Input:**
```
- foo

→→bar
```
**Output:**
```html
<ul>
<li>
<p>foo</p>
<pre><code>  bar
</code></pre>
</li>
</ul>
```

## Tabs

[CM-006] Example 6
**Input:**
```
>→→foo
```
**Output:**
```html
<blockquote>
<pre><code>  foo
</code></pre>
</blockquote>
```

## Tabs

[CM-007] Example 7
**Input:**
```
-→→foo
```
**Output:**
```html
<ul>
<li>
<pre><code>  foo
</code></pre>
</li>
</ul>
```

## Tabs

[CM-008] Example 8
**Input:**
```
    foo
→bar
```
**Output:**
```html
<pre><code>foo
bar
</code></pre>
```

## Tabs

[CM-009] Example 9
**Input:**
```
 - foo
   - bar
→ - baz
```
**Output:**
```html
<ul>
<li>foo
<ul>
<li>bar
<ul>
<li>baz</li>
</ul>
</li>
</ul>
</li>
</ul>
```

## Tabs

[CM-010] Example 10
**Input:**
```
#→Foo
```
**Output:**
```html
<h1>Foo</h1>
```

## Tabs

[CM-011] Example 11
**Input:**
```
*→*→*→
```
**Output:**
```html
<hr />
```

## Backslash escapes

[CM-012] Example 12
**Input:**
```
\!\"\#\$\%\&\'\(\)\*\+\,\-\.\/\:\;\<\=\>\?\@\[\\\]\^\_\`\{\|\}\~
```
**Output:**
```html
<p>!&quot;#$%&amp;'()*+,-./:;&lt;=&gt;?@[\]^_`{|}~</p>
```

## Backslash escapes

[CM-013] Example 13
**Input:**
```
\→\A\a\ \3\φ\«
```
**Output:**
```html
<p>\→\A\a\ \3\φ\«</p>
```

## Backslash escapes

[CM-014] Example 14
**Input:**
```
\*not emphasized*
\<br/> not a tag
\[not a link](/foo)
\`not code`
1\. not a list
\* not a list
\# not a heading
\[foo]: /url "not a reference"
\&ouml; not a character entity
```
**Output:**
```html
<p>*not emphasized*
&lt;br/&gt; not a tag
[not a link](/foo)
`not code`
1. not a list
* not a list
# not a heading
[foo]: /url &quot;not a reference&quot;
&amp;ouml; not a character entity</p>
```

## Backslash escapes

[CM-015] Example 15
**Input:**
```
\\*emphasis*
```
**Output:**
```html
<p>\<em>emphasis</em></p>
```

## Backslash escapes

[CM-016] Example 16
**Input:**
```
foo\
bar
```
**Output:**
```html
<p>foo<br />
bar</p>
```

## Backslash escapes

[CM-017] Example 17
**Input:**
```
`` \[\` ``
```
**Output:**
```html
<p><code>\[\`</code></p>
```

## Backslash escapes

[CM-018] Example 18
**Input:**
```
    \[\]
```
**Output:**
```html
<pre><code>\[\]
</code></pre>
```

## Backslash escapes

[CM-019] Example 19
**Input:**
```
~~~
\[\]
~~~
```
**Output:**
```html
<pre><code>\[\]
</code></pre>
```

## Backslash escapes

[CM-020] Example 20
**Input:**
```
<https://example.com?find=\*>
```
**Output:**
```html
<p><a href="https://example.com?find=%5C*">https://example.com?find=\*</a></p>
```

## Backslash escapes

[CM-021] Example 21
**Input:**
```
<a href="/bar\/)">
```
**Output:**
```html
<a href="/bar\/)">
```

## Backslash escapes

[CM-022] Example 22
**Input:**
```
[foo](/bar\* "ti\*tle")
```
**Output:**
```html
<p><a href="/bar*" title="ti*tle">foo</a></p>
```

## Backslash escapes

[CM-023] Example 23
**Input:**
```
[foo]

[foo]: /bar\* "ti\*tle"
```
**Output:**
```html
<p><a href="/bar*" title="ti*tle">foo</a></p>
```

## Backslash escapes

[CM-024] Example 24
**Input:**
```
``` foo\+bar
foo
```
```
**Output:**
```html
<pre><code class="language-foo+bar">foo
</code></pre>
```

## Entity and numeric character references

[CM-025] Example 25
**Input:**
```
&nbsp; &amp; &copy; &AElig; &Dcaron;
&frac34; &HilbertSpace; &DifferentialD;
&ClockwiseContourIntegral; &ngE;
```
**Output:**
```html
<p>  &amp; © Æ Ď
¾ ℋ ⅆ
∲ ≧̸</p>
```

## Entity and numeric character references

[CM-026] Example 26
**Input:**
```
&#35; &#1234; &#992; &#0;
```
**Output:**
```html
<p># Ӓ Ϡ �</p>
```

## Entity and numeric character references

[CM-027] Example 27
**Input:**
```
&#X22; &#XD06; &#xcab;
```
**Output:**
```html
<p>&quot; ആ ಫ</p>
```

## Entity and numeric character references

[CM-028] Example 28
**Input:**
```
&nbsp &x; &#; &#x;
&#87654321;
&#abcdef0;
&ThisIsNotDefined; &hi?;
```
**Output:**
```html
<p>&amp;nbsp &amp;x; &amp;#; &amp;#x;
&amp;#87654321;
&amp;#abcdef0;
&amp;ThisIsNotDefined; &amp;hi?;</p>
```

## Entity and numeric character references

[CM-029] Example 29
**Input:**
```
&copy
```
**Output:**
```html
<p>&amp;copy</p>
```

## Entity and numeric character references

[CM-030] Example 30
**Input:**
```
&MadeUpEntity;
```
**Output:**
```html
<p>&amp;MadeUpEntity;</p>
```

## Entity and numeric character references

[CM-031] Example 31
**Input:**
```
<a href="&ouml;&ouml;.html">
```
**Output:**
```html
<a href="&ouml;&ouml;.html">
```

## Entity and numeric character references

[CM-032] Example 32
**Input:**
```
[foo](/f&ouml;&ouml; "f&ouml;&ouml;")
```
**Output:**
```html
<p><a href="/f%C3%B6%C3%B6" title="föö">foo</a></p>
```

## Entity and numeric character references

[CM-033] Example 33
**Input:**
```
[foo]

[foo]: /f&ouml;&ouml; "f&ouml;&ouml;"
```
**Output:**
```html
<p><a href="/f%C3%B6%C3%B6" title="föö">foo</a></p>
```

## Entity and numeric character references

[CM-034] Example 34
**Input:**
```
``` f&ouml;&ouml;
foo
```
```
**Output:**
```html
<pre><code class="language-föö">foo
</code></pre>
```

## Entity and numeric character references

[CM-035] Example 35
**Input:**
```
`f&ouml;&ouml;`
```
**Output:**
```html
<p><code>f&amp;ouml;&amp;ouml;</code></p>
```

## Entity and numeric character references

[CM-036] Example 36
**Input:**
```
    f&ouml;f&ouml;
```
**Output:**
```html
<pre><code>f&amp;ouml;f&amp;ouml;
</code></pre>
```

## Entity and numeric character references

[CM-037] Example 37
**Input:**
```
&#42;foo&#42;
*foo*
```
**Output:**
```html
<p>*foo*
<em>foo</em></p>
```

## Entity and numeric character references

[CM-038] Example 38
**Input:**
```
&#42; foo

* foo
```
**Output:**
```html
<p>* foo</p>
<ul>
<li>foo</li>
</ul>
```

## Entity and numeric character references

[CM-039] Example 39
**Input:**
```
foo&#10;&#10;bar
```
**Output:**
```html
<p>foo

bar</p>
```

## Entity and numeric character references

[CM-040] Example 40
**Input:**
```
&#9;foo
```
**Output:**
```html
<p>→foo</p>
```

## Entity and numeric character references

[CM-041] Example 41
**Input:**
```
[a](url &quot;tit&quot;)
```
**Output:**
```html
<p>[a](url &quot;tit&quot;)</p>
```

## Precedence

[CM-042] Example 42
**Input:**
```
- `one
- two`
```
**Output:**
```html
<ul>
<li>`one</li>
<li>two`</li>
</ul>
```

## Thematic breaks

[CM-043] Example 43
**Input:**
```
***
---
___
```
**Output:**
```html
<hr />
<hr />
<hr />
```

## Thematic breaks

[CM-044] Example 44
**Input:**
```
+++
```
**Output:**
```html
<p>+++</p>
```

## Thematic breaks

[CM-045] Example 45
**Input:**
```
===
```
**Output:**
```html
<p>===</p>
```

## Thematic breaks

[CM-046] Example 46
**Input:**
```
--
**
__
```
**Output:**
```html
<p>--
**
__</p>
```

## Thematic breaks

[CM-047] Example 47
**Input:**
```
 ***
  ***
   ***
```
**Output:**
```html
<hr />
<hr />
<hr />
```

## Thematic breaks

[CM-048] Example 48
**Input:**
```
    ***
```
**Output:**
```html
<pre><code>***
</code></pre>
```

## Thematic breaks

[CM-049] Example 49
**Input:**
```
Foo
    ***
```
**Output:**
```html
<p>Foo
***</p>
```

## Thematic breaks

[CM-050] Example 50
**Input:**
```
_____________________________________
```
**Output:**
```html
<hr />
```

## Thematic breaks

[CM-051] Example 51
**Input:**
```
 - - -
```
**Output:**
```html
<hr />
```

## Thematic breaks

[CM-052] Example 52
**Input:**
```
 **  * ** * ** * **
```
**Output:**
```html
<hr />
```

## Thematic breaks

[CM-053] Example 53
**Input:**
```
-     -      -      -
```
**Output:**
```html
<hr />
```

## Thematic breaks

[CM-054] Example 54
**Input:**
```
- - - -    
```
**Output:**
```html
<hr />
```

## Thematic breaks

[CM-055] Example 55
**Input:**
```
_ _ _ _ a

a------

---a---
```
**Output:**
```html
<p>_ _ _ _ a</p>
<p>a------</p>
<p>---a---</p>
```

## Thematic breaks

[CM-056] Example 56
**Input:**
```
 *-*
```
**Output:**
```html
<p><em>-</em></p>
```

## Thematic breaks

[CM-057] Example 57
**Input:**
```
- foo
***
- bar
```
**Output:**
```html
<ul>
<li>foo</li>
</ul>
<hr />
<ul>
<li>bar</li>
</ul>
```

## Thematic breaks

[CM-058] Example 58
**Input:**
```
Foo
***
bar
```
**Output:**
```html
<p>Foo</p>
<hr />
<p>bar</p>
```

## Thematic breaks

[CM-059] Example 59
**Input:**
```
Foo
---
bar
```
**Output:**
```html
<h2>Foo</h2>
<p>bar</p>
```

## Thematic breaks

[CM-060] Example 60
**Input:**
```
* Foo
* * *
* Bar
```
**Output:**
```html
<ul>
<li>Foo</li>
</ul>
<hr />
<ul>
<li>Bar</li>
</ul>
```

## Thematic breaks

[CM-061] Example 61
**Input:**
```
- Foo
- * * *
```
**Output:**
```html
<ul>
<li>Foo</li>
<li>
<hr />
</li>
</ul>
```

## ATX headings

[CM-062] Example 62
**Input:**
```
# foo
## foo
### foo
#### foo
##### foo
###### foo
```
**Output:**
```html
<h1>foo</h1>
<h2>foo</h2>
<h3>foo</h3>
<h4>foo</h4>
<h5>foo</h5>
<h6>foo</h6>
```

## ATX headings

[CM-063] Example 63
**Input:**
```
####### foo
```
**Output:**
```html
<p>####### foo</p>
```

## ATX headings

[CM-064] Example 64
**Input:**
```
#5 bolt

#hashtag
```
**Output:**
```html
<p>#5 bolt</p>
<p>#hashtag</p>
```

## ATX headings

[CM-065] Example 65
**Input:**
```
\## foo
```
**Output:**
```html
<p>## foo</p>
```

## ATX headings

[CM-066] Example 66
**Input:**
```
# foo *bar* \*baz\*
```
**Output:**
```html
<h1>foo <em>bar</em> *baz*</h1>
```

## ATX headings

[CM-067] Example 67
**Input:**
```
#                  foo                     
```
**Output:**
```html
<h1>foo</h1>
```

## ATX headings

[CM-068] Example 68
**Input:**
```
 ### foo
  ## foo
   # foo
```
**Output:**
```html
<h3>foo</h3>
<h2>foo</h2>
<h1>foo</h1>
```

## ATX headings

[CM-069] Example 69
**Input:**
```
    # foo
```
**Output:**
```html
<pre><code># foo
</code></pre>
```

## ATX headings

[CM-070] Example 70
**Input:**
```
foo
    # bar
```
**Output:**
```html
<p>foo
# bar</p>
```

## ATX headings

[CM-071] Example 71
**Input:**
```
## foo ##
  ###   bar    ###
```
**Output:**
```html
<h2>foo</h2>
<h3>bar</h3>
```

## ATX headings

[CM-072] Example 72
**Input:**
```
# foo ##################################
##### foo ##
```
**Output:**
```html
<h1>foo</h1>
<h5>foo</h5>
```

## ATX headings

[CM-073] Example 73
**Input:**
```
### foo ###     
```
**Output:**
```html
<h3>foo</h3>
```

## ATX headings

[CM-074] Example 74
**Input:**
```
### foo ### b
```
**Output:**
```html
<h3>foo ### b</h3>
```

## ATX headings

[CM-075] Example 75
**Input:**
```
# foo#
```
**Output:**
```html
<h1>foo#</h1>
```

## ATX headings

[CM-076] Example 76
**Input:**
```
### foo \###
## foo #\##
# foo \#
```
**Output:**
```html
<h3>foo ###</h3>
<h2>foo ###</h2>
<h1>foo #</h1>
```

## ATX headings

[CM-077] Example 77
**Input:**
```
****
## foo
****
```
**Output:**
```html
<hr />
<h2>foo</h2>
<hr />
```

## ATX headings

[CM-078] Example 78
**Input:**
```
Foo bar
# baz
Bar foo
```
**Output:**
```html
<p>Foo bar</p>
<h1>baz</h1>
<p>Bar foo</p>
```

## ATX headings

[CM-079] Example 79
**Input:**
```
## 
#
### ###
```
**Output:**
```html
<h2></h2>
<h1></h1>
<h3></h3>
```

## Setext headings

[CM-080] Example 80
**Input:**
```
Foo *bar*
=========

Foo *bar*
---------
```
**Output:**
```html
<h1>Foo <em>bar</em></h1>
<h2>Foo <em>bar</em></h2>
```

## Setext headings

[CM-081] Example 81
**Input:**
```
Foo *bar
baz*
====
```
**Output:**
```html
<h1>Foo <em>bar
baz</em></h1>
```

## Setext headings

[CM-082] Example 82
**Input:**
```
  Foo *bar
baz*→
====
```
**Output:**
```html
<h1>Foo <em>bar
baz</em></h1>
```

## Setext headings

[CM-083] Example 83
**Input:**
```
Foo
-------------------------

Foo
=
```
**Output:**
```html
<h2>Foo</h2>
<h1>Foo</h1>
```

## Setext headings

[CM-084] Example 84
**Input:**
```
   Foo
---

  Foo
-----

  Foo
  ===
```
**Output:**
```html
<h2>Foo</h2>
<h2>Foo</h2>
<h1>Foo</h1>
```

## Setext headings

[CM-085] Example 85
**Input:**
```
    Foo
    ---

    Foo
---
```
**Output:**
```html
<pre><code>Foo
---

Foo
</code></pre>
<hr />
```

## Setext headings

[CM-086] Example 86
**Input:**
```
Foo
   ----      
```
**Output:**
```html
<h2>Foo</h2>
```

## Setext headings

[CM-087] Example 87
**Input:**
```
Foo
    ---
```
**Output:**
```html
<p>Foo
---</p>
```

## Setext headings

[CM-088] Example 88
**Input:**
```
Foo
= =

Foo
--- -
```
**Output:**
```html
<p>Foo
= =</p>
<p>Foo</p>
<hr />
```

## Setext headings

[CM-089] Example 89
**Input:**
```
Foo  
-----
```
**Output:**
```html
<h2>Foo</h2>
```

## Setext headings

[CM-090] Example 90
**Input:**
```
Foo\
----
```
**Output:**
```html
<h2>Foo\</h2>
```

## Setext headings

[CM-091] Example 91
**Input:**
```
`Foo
----
`

<a title="a lot
---
of dashes"/>
```
**Output:**
```html
<h2>`Foo</h2>
<p>`</p>
<h2>&lt;a title=&quot;a lot</h2>
<p>of dashes&quot;/&gt;</p>
```

## Setext headings

[CM-092] Example 92
**Input:**
```
> Foo
---
```
**Output:**
```html
<blockquote>
<p>Foo</p>
</blockquote>
<hr />
```

## Setext headings

[CM-093] Example 93
**Input:**
```
> foo
bar
===
```
**Output:**
```html
<blockquote>
<p>foo
bar
===</p>
</blockquote>
```

## Setext headings

[CM-094] Example 94
**Input:**
```
- Foo
---
```
**Output:**
```html
<ul>
<li>Foo</li>
</ul>
<hr />
```

## Setext headings

[CM-095] Example 95
**Input:**
```
Foo
Bar
---
```
**Output:**
```html
<h2>Foo
Bar</h2>
```

## Setext headings

[CM-096] Example 96
**Input:**
```
---
Foo
---
Bar
---
Baz
```
**Output:**
```html
<hr />
<h2>Foo</h2>
<h2>Bar</h2>
<p>Baz</p>
```

## Setext headings

[CM-097] Example 97
**Input:**
```

====
```
**Output:**
```html
<p>====</p>
```

## Setext headings

[CM-098] Example 98
**Input:**
```
---
---
```
**Output:**
```html
<hr />
<hr />
```

## Setext headings

[CM-099] Example 99
**Input:**
```
- foo
-----
```
**Output:**
```html
<ul>
<li>foo</li>
</ul>
<hr />
```

## Setext headings

[CM-100] Example 100
**Input:**
```
    foo
---
```
**Output:**
```html
<pre><code>foo
</code></pre>
<hr />
```

## Setext headings

[CM-101] Example 101
**Input:**
```
> foo
-----
```
**Output:**
```html
<blockquote>
<p>foo</p>
</blockquote>
<hr />
```

## Setext headings

[CM-102] Example 102
**Input:**
```
\> foo
------
```
**Output:**
```html
<h2>&gt; foo</h2>
```

## Setext headings

[CM-103] Example 103
**Input:**
```
Foo

bar
---
baz
```
**Output:**
```html
<p>Foo</p>
<h2>bar</h2>
<p>baz</p>
```

## Setext headings

[CM-104] Example 104
**Input:**
```
Foo
bar

---

baz
```
**Output:**
```html
<p>Foo
bar</p>
<hr />
<p>baz</p>
```

## Setext headings

[CM-105] Example 105
**Input:**
```
Foo
bar
* * *
baz
```
**Output:**
```html
<p>Foo
bar</p>
<hr />
<p>baz</p>
```

## Setext headings

[CM-106] Example 106
**Input:**
```
Foo
bar
\---
baz
```
**Output:**
```html
<p>Foo
bar
---
baz</p>
```

## Indented code blocks

[CM-107] Example 107
**Input:**
```
    a simple
      indented code block
```
**Output:**
```html
<pre><code>a simple
  indented code block
</code></pre>
```

## Indented code blocks

[CM-108] Example 108
**Input:**
```
  - foo

    bar
```
**Output:**
```html
<ul>
<li>
<p>foo</p>
<p>bar</p>
</li>
</ul>
```

## Indented code blocks

[CM-109] Example 109
**Input:**
```
1.  foo

    - bar
```
**Output:**
```html
<ol>
<li>
<p>foo</p>
<ul>
<li>bar</li>
</ul>
</li>
</ol>
```

## Indented code blocks

[CM-110] Example 110
**Input:**
```
    <a/>
    *hi*

    - one
```
**Output:**
```html
<pre><code>&lt;a/&gt;
*hi*

- one
</code></pre>
```

## Indented code blocks

[CM-111] Example 111
**Input:**
```
    chunk1

    chunk2
  
 
 
    chunk3
```
**Output:**
```html
<pre><code>chunk1

chunk2



chunk3
</code></pre>
```

## Indented code blocks

[CM-112] Example 112
**Input:**
```
    chunk1
      
      chunk2
```
**Output:**
```html
<pre><code>chunk1
  
  chunk2
</code></pre>
```

## Indented code blocks

[CM-113] Example 113
**Input:**
```
Foo
    bar

```
**Output:**
```html
<p>Foo
bar</p>
```

## Indented code blocks

[CM-114] Example 114
**Input:**
```
    foo
bar
```
**Output:**
```html
<pre><code>foo
</code></pre>
<p>bar</p>
```

## Indented code blocks

[CM-115] Example 115
**Input:**
```
# Heading
    foo
Heading
------
    foo
----
```
**Output:**
```html
<h1>Heading</h1>
<pre><code>foo
</code></pre>
<h2>Heading</h2>
<pre><code>foo
</code></pre>
<hr />
```

## Indented code blocks

[CM-116] Example 116
**Input:**
```
        foo
    bar
```
**Output:**
```html
<pre><code>    foo
bar
</code></pre>
```

## Indented code blocks

[CM-117] Example 117
**Input:**
```

    
    foo
    

```
**Output:**
```html
<pre><code>foo
</code></pre>
```

## Indented code blocks

[CM-118] Example 118
**Input:**
```
    foo  
```
**Output:**
```html
<pre><code>foo  
</code></pre>
```

## Fenced code blocks

[CM-119] Example 119
**Input:**
```
```
<
 >
```
```
**Output:**
```html
<pre><code>&lt;
 &gt;
</code></pre>
```

## Fenced code blocks

[CM-120] Example 120
**Input:**
```
~~~
<
 >
~~~
```
**Output:**
```html
<pre><code>&lt;
 &gt;
</code></pre>
```

## Fenced code blocks

[CM-121] Example 121
**Input:**
```
``
foo
``
```
**Output:**
```html
<p><code>foo</code></p>
```

## Fenced code blocks

[CM-122] Example 122
**Input:**
```
```
aaa
~~~
```
```
**Output:**
```html
<pre><code>aaa
~~~
</code></pre>
```

## Fenced code blocks

[CM-123] Example 123
**Input:**
```
~~~
aaa
```
~~~
```
**Output:**
```html
<pre><code>aaa
```
</code></pre>
```

## Fenced code blocks

[CM-124] Example 124
**Input:**
```
````
aaa
```
``````
```
**Output:**
```html
<pre><code>aaa
```
</code></pre>
```

## Fenced code blocks

[CM-125] Example 125
**Input:**
```
~~~~
aaa
~~~
~~~~
```
**Output:**
```html
<pre><code>aaa
~~~
</code></pre>
```

## Fenced code blocks

[CM-126] Example 126
**Input:**
```
```
```
**Output:**
```html
<pre><code></code></pre>
```

## Fenced code blocks

[CM-127] Example 127
**Input:**
```
`````

```
aaa
```
**Output:**
```html
<pre><code>
```
aaa
</code></pre>
```

## Fenced code blocks

[CM-128] Example 128
**Input:**
```
> ```
> aaa

bbb
```
**Output:**
```html
<blockquote>
<pre><code>aaa
</code></pre>
</blockquote>
<p>bbb</p>
```

## Fenced code blocks

[CM-129] Example 129
**Input:**
```
```

  
```
```
**Output:**
```html
<pre><code>
  
</code></pre>
```

## Fenced code blocks

[CM-130] Example 130
**Input:**
```
```
```
```
**Output:**
```html
<pre><code></code></pre>
```

## Fenced code blocks

[CM-131] Example 131
**Input:**
```
 ```
 aaa
aaa
```
```
**Output:**
```html
<pre><code>aaa
aaa
</code></pre>
```

## Fenced code blocks

[CM-132] Example 132
**Input:**
```
  ```
aaa
  aaa
aaa
  ```
```
**Output:**
```html
<pre><code>aaa
aaa
aaa
</code></pre>
```

## Fenced code blocks

[CM-133] Example 133
**Input:**
```
   ```
   aaa
    aaa
  aaa
   ```
```
**Output:**
```html
<pre><code>aaa
 aaa
aaa
</code></pre>
```

## Fenced code blocks

[CM-134] Example 134
**Input:**
```
    ```
    aaa
    ```
```
**Output:**
```html
<pre><code>```
aaa
```
</code></pre>
```

## Fenced code blocks

[CM-135] Example 135
**Input:**
```
```
aaa
  ```
```
**Output:**
```html
<pre><code>aaa
</code></pre>
```

## Fenced code blocks

[CM-136] Example 136
**Input:**
```
   ```
aaa
  ```
```
**Output:**
```html
<pre><code>aaa
</code></pre>
```

## Fenced code blocks

[CM-137] Example 137
**Input:**
```
```
aaa
    ```
```
**Output:**
```html
<pre><code>aaa
    ```
</code></pre>
```

## Fenced code blocks

[CM-138] Example 138
**Input:**
```
``` ```
aaa
```
**Output:**
```html
<p><code> </code>
aaa</p>
```

## Fenced code blocks

[CM-139] Example 139
**Input:**
```
~~~~~~
aaa
~~~ ~~
```
**Output:**
```html
<pre><code>aaa
~~~ ~~
</code></pre>
```

## Fenced code blocks

[CM-140] Example 140
**Input:**
```
foo
```
bar
```
baz
```
**Output:**
```html
<p>foo</p>
<pre><code>bar
</code></pre>
<p>baz</p>
```

## Fenced code blocks

[CM-141] Example 141
**Input:**
```
foo
---
~~~
bar
~~~
# baz
```
**Output:**
```html
<h2>foo</h2>
<pre><code>bar
</code></pre>
<h1>baz</h1>
```

## Fenced code blocks

[CM-142] Example 142
**Input:**
```
```ruby
def foo(x)
  return 3
end
```
```
**Output:**
```html
<pre><code class="language-ruby">def foo(x)
  return 3
end
</code></pre>
```

## Fenced code blocks

[CM-143] Example 143
**Input:**
```
~~~~    ruby startline=3 $%@#$
def foo(x)
  return 3
end
~~~~~~~
```
**Output:**
```html
<pre><code class="language-ruby">def foo(x)
  return 3
end
</code></pre>
```

## Fenced code blocks

[CM-144] Example 144
**Input:**
```
````;
````
```
**Output:**
```html
<pre><code class="language-;"></code></pre>
```

## Fenced code blocks

[CM-145] Example 145
**Input:**
```
``` aa ```
foo
```
**Output:**
```html
<p><code>aa</code>
foo</p>
```

## Fenced code blocks

[CM-146] Example 146
**Input:**
```
~~~ aa ``` ~~~
foo
~~~
```
**Output:**
```html
<pre><code class="language-aa">foo
</code></pre>
```

## Fenced code blocks

[CM-147] Example 147
**Input:**
```
```
``` aaa
```
```
**Output:**
```html
<pre><code>``` aaa
</code></pre>
```

## HTML blocks

[CM-148] Example 148
**Input:**
```
<table><tr><td>
<pre>
**Hello**,

_world_.
</pre>
</td></tr></table>
```
**Output:**
```html
<table><tr><td>
<pre>
**Hello**,
<p><em>world</em>.
</pre></p>
</td></tr></table>
```

## HTML blocks

[CM-149] Example 149
**Input:**
```
<table>
  <tr>
    <td>
           hi
    </td>
  </tr>
</table>

okay.
```
**Output:**
```html
<table>
  <tr>
    <td>
           hi
    </td>
  </tr>
</table>
<p>okay.</p>
```

## HTML blocks

[CM-150] Example 150
**Input:**
```
 <div>
  *hello*
         <foo><a>
```
**Output:**
```html
 <div>
  *hello*
         <foo><a>
```

## HTML blocks

[CM-151] Example 151
**Input:**
```
</div>
*foo*
```
**Output:**
```html
</div>
*foo*
```

## HTML blocks

[CM-152] Example 152
**Input:**
```
<DIV CLASS="foo">

*Markdown*

</DIV>
```
**Output:**
```html
<DIV CLASS="foo">
<p><em>Markdown</em></p>
</DIV>
```

## HTML blocks

[CM-153] Example 153
**Input:**
```
<div id="foo"
  class="bar">
</div>
```
**Output:**
```html
<div id="foo"
  class="bar">
</div>
```

## HTML blocks

[CM-154] Example 154
**Input:**
```
<div id="foo" class="bar
  baz">
</div>
```
**Output:**
```html
<div id="foo" class="bar
  baz">
</div>
```

## HTML blocks

[CM-155] Example 155
**Input:**
```
<div>
*foo*

*bar*
```
**Output:**
```html
<div>
*foo*
<p><em>bar</em></p>
```

## HTML blocks

[CM-156] Example 156
**Input:**
```
<div id="foo"
*hi*
```
**Output:**
```html
<div id="foo"
*hi*
```

## HTML blocks

[CM-157] Example 157
**Input:**
```
<div class
foo
```
**Output:**
```html
<div class
foo
```

## HTML blocks

[CM-158] Example 158
**Input:**
```
<div *???-&&&-<---
*foo*
```
**Output:**
```html
<div *???-&&&-<---
*foo*
```

## HTML blocks

[CM-159] Example 159
**Input:**
```
<div><a href="bar">*foo*</a></div>
```
**Output:**
```html
<div><a href="bar">*foo*</a></div>
```

## HTML blocks

[CM-160] Example 160
**Input:**
```
<table><tr><td>
foo
</td></tr></table>
```
**Output:**
```html
<table><tr><td>
foo
</td></tr></table>
```

## HTML blocks

[CM-161] Example 161
**Input:**
```
<div></div>
``` c
int x = 33;
```
```
**Output:**
```html
<div></div>
``` c
int x = 33;
```
```

## HTML blocks

[CM-162] Example 162
**Input:**
```
<a href="foo">
*bar*
</a>
```
**Output:**
```html
<a href="foo">
*bar*
</a>
```

## HTML blocks

[CM-163] Example 163
**Input:**
```
<Warning>
*bar*
</Warning>
```
**Output:**
```html
<Warning>
*bar*
</Warning>
```

## HTML blocks

[CM-164] Example 164
**Input:**
```
<i class="foo">
*bar*
</i>
```
**Output:**
```html
<i class="foo">
*bar*
</i>
```

## HTML blocks

[CM-165] Example 165
**Input:**
```
</ins>
*bar*
```
**Output:**
```html
</ins>
*bar*
```

## HTML blocks

[CM-166] Example 166
**Input:**
```
<del>
*foo*
</del>
```
**Output:**
```html
<del>
*foo*
</del>
```

## HTML blocks

[CM-167] Example 167
**Input:**
```
<del>

*foo*

</del>
```
**Output:**
```html
<del>
<p><em>foo</em></p>
</del>
```

## HTML blocks

[CM-168] Example 168
**Input:**
```
<del>*foo*</del>
```
**Output:**
```html
<p><del><em>foo</em></del></p>
```

## HTML blocks

[CM-169] Example 169
**Input:**
```
<pre language="haskell"><code>
import Text.HTML.TagSoup

main :: IO ()
main = print $ parseTags tags
</code></pre>
okay
```
**Output:**
```html
<pre language="haskell"><code>
import Text.HTML.TagSoup

main :: IO ()
main = print $ parseTags tags
</code></pre>
<p>okay</p>
```

## HTML blocks

[CM-170] Example 170
**Input:**
```
<script type="text/javascript">
// JavaScript example

document.getElementById("demo").innerHTML = "Hello JavaScript!";
</script>
okay
```
**Output:**
```html
<script type="text/javascript">
// JavaScript example

document.getElementById("demo").innerHTML = "Hello JavaScript!";
</script>
<p>okay</p>
```

## HTML blocks

[CM-171] Example 171
**Input:**
```
<textarea>

*foo*

_bar_

</textarea>
```
**Output:**
```html
<textarea>

*foo*

_bar_

</textarea>
```

## HTML blocks

[CM-172] Example 172
**Input:**
```
<style
  type="text/css">
h1 {color:red;}

p {color:blue;}
</style>
okay
```
**Output:**
```html
<style
  type="text/css">
h1 {color:red;}

p {color:blue;}
</style>
<p>okay</p>
```

## HTML blocks

[CM-173] Example 173
**Input:**
```
<style
  type="text/css">

foo
```
**Output:**
```html
<style
  type="text/css">

foo
```

## HTML blocks

[CM-174] Example 174
**Input:**
```
> <div>
> foo

bar
```
**Output:**
```html
<blockquote>
<div>
foo
</blockquote>
<p>bar</p>
```

## HTML blocks

[CM-175] Example 175
**Input:**
```
- <div>
- foo
```
**Output:**
```html
<ul>
<li>
<div>
</li>
<li>foo</li>
</ul>
```

## HTML blocks

[CM-176] Example 176
**Input:**
```
<style>p{color:red;}</style>
*foo*
```
**Output:**
```html
<style>p{color:red;}</style>
<p><em>foo</em></p>
```

## HTML blocks

[CM-177] Example 177
**Input:**
```
<!-- foo -->*bar*
*baz*
```
**Output:**
```html
<!-- foo -->*bar*
<p><em>baz</em></p>
```

## HTML blocks

[CM-178] Example 178
**Input:**
```
<script>
foo
</script>1. *bar*
```
**Output:**
```html
<script>
foo
</script>1. *bar*
```

## HTML blocks

[CM-179] Example 179
**Input:**
```
<!-- Foo

bar
   baz -->
okay
```
**Output:**
```html
<!-- Foo

bar
   baz -->
<p>okay</p>
```

## HTML blocks

[CM-180] Example 180
**Input:**
```
<?php

  echo '>';

?>
okay
```
**Output:**
```html
<?php

  echo '>';

?>
<p>okay</p>
```

## HTML blocks

[CM-181] Example 181
**Input:**
```
<!DOCTYPE html>
```
**Output:**
```html
<!DOCTYPE html>
```

## HTML blocks

[CM-182] Example 182
**Input:**
```
<![CDATA[
function matchwo(a,b)
{
  if (a < b && a < 0) then {
    return 1;

  } else {

    return 0;
  }
}
]]>
okay
```
**Output:**
```html
<![CDATA[
function matchwo(a,b)
{
  if (a < b && a < 0) then {
    return 1;

  } else {

    return 0;
  }
}
]]>
<p>okay</p>
```

## HTML blocks

[CM-183] Example 183
**Input:**
```
  <!-- foo -->

    <!-- foo -->
```
**Output:**
```html
  <!-- foo -->
<pre><code>&lt;!-- foo --&gt;
</code></pre>
```

## HTML blocks

[CM-184] Example 184
**Input:**
```
  <div>

    <div>
```
**Output:**
```html
  <div>
<pre><code>&lt;div&gt;
</code></pre>
```

## HTML blocks

[CM-185] Example 185
**Input:**
```
Foo
<div>
bar
</div>
```
**Output:**
```html
<p>Foo</p>
<div>
bar
</div>
```

## HTML blocks

[CM-186] Example 186
**Input:**
```
<div>
bar
</div>
*foo*
```
**Output:**
```html
<div>
bar
</div>
*foo*
```

## HTML blocks

[CM-187] Example 187
**Input:**
```
Foo
<a href="bar">
baz
```
**Output:**
```html
<p>Foo
<a href="bar">
baz</p>
```

## HTML blocks

[CM-188] Example 188
**Input:**
```
<div>

*Emphasized* text.

</div>
```
**Output:**
```html
<div>
<p><em>Emphasized</em> text.</p>
</div>
```

## HTML blocks

[CM-189] Example 189
**Input:**
```
<div>
*Emphasized* text.
</div>
```
**Output:**
```html
<div>
*Emphasized* text.
</div>
```

## HTML blocks

[CM-190] Example 190
**Input:**
```
<table>

<tr>

<td>
Hi
</td>

</tr>

</table>
```
**Output:**
```html
<table>
<tr>
<td>
Hi
</td>
</tr>
</table>
```

## HTML blocks

[CM-191] Example 191
**Input:**
```
<table>

  <tr>

    <td>
      Hi
    </td>

  </tr>

</table>
```
**Output:**
```html
<table>
  <tr>
<pre><code>&lt;td&gt;
  Hi
&lt;/td&gt;
</code></pre>
  </tr>
</table>
```

## Link reference definitions

[CM-192] Example 192
**Input:**
```
[foo]: /url "title"

[foo]
```
**Output:**
```html
<p><a href="/url" title="title">foo</a></p>
```

## Link reference definitions

[CM-193] Example 193
**Input:**
```
   [foo]: 
      /url  
           'the title'  

[foo]
```
**Output:**
```html
<p><a href="/url" title="the title">foo</a></p>
```

## Link reference definitions

[CM-194] Example 194
**Input:**
```
[Foo*bar\]]:my_(url) 'title (with parens)'

[Foo*bar\]]
```
**Output:**
```html
<p><a href="my_(url)" title="title (with parens)">Foo*bar]</a></p>
```

## Link reference definitions

[CM-195] Example 195
**Input:**
```
[Foo bar]:
<my url>
'title'

[Foo bar]
```
**Output:**
```html
<p><a href="my%20url" title="title">Foo bar</a></p>
```

## Link reference definitions

[CM-196] Example 196
**Input:**
```
[foo]: /url '
title
line1
line2
'

[foo]
```
**Output:**
```html
<p><a href="/url" title="
title
line1
line2
">foo</a></p>
```

## Link reference definitions

[CM-197] Example 197
**Input:**
```
[foo]: /url 'title

with blank line'

[foo]
```
**Output:**
```html
<p>[foo]: /url 'title</p>
<p>with blank line'</p>
<p>[foo]</p>
```

## Link reference definitions

[CM-198] Example 198
**Input:**
```
[foo]:
/url

[foo]
```
**Output:**
```html
<p><a href="/url">foo</a></p>
```

## Link reference definitions

[CM-199] Example 199
**Input:**
```
[foo]:

[foo]
```
**Output:**
```html
<p>[foo]:</p>
<p>[foo]</p>
```

## Link reference definitions

[CM-200] Example 200
**Input:**
```
[foo]: <>

[foo]
```
**Output:**
```html
<p><a href="">foo</a></p>
```

## Link reference definitions

[CM-201] Example 201
**Input:**
```
[foo]: <bar>(baz)

[foo]
```
**Output:**
```html
<p>[foo]: <bar>(baz)</p>
<p>[foo]</p>
```

## Link reference definitions

[CM-202] Example 202
**Input:**
```
[foo]: /url\bar\*baz "foo\"bar\baz"

[foo]
```
**Output:**
```html
<p><a href="/url%5Cbar*baz" title="foo&quot;bar\baz">foo</a></p>
```

## Link reference definitions

[CM-203] Example 203
**Input:**
```
[foo]

[foo]: url
```
**Output:**
```html
<p><a href="url">foo</a></p>
```

## Link reference definitions

[CM-204] Example 204
**Input:**
```
[foo]

[foo]: first
[foo]: second
```
**Output:**
```html
<p><a href="first">foo</a></p>
```

## Link reference definitions

[CM-205] Example 205
**Input:**
```
[FOO]: /url

[Foo]
```
**Output:**
```html
<p><a href="/url">Foo</a></p>
```

## Link reference definitions

[CM-206] Example 206
**Input:**
```
[ΑΓΩ]: /φου

[αγω]
```
**Output:**
```html
<p><a href="/%CF%86%CE%BF%CF%85">αγω</a></p>
```

## Link reference definitions

[CM-207] Example 207
**Input:**
```
[foo]: /url
```
**Output:**
```html
.
```

## Link reference definitions

[CM-208] Example 208
**Input:**
```
[
foo
]: /url
bar
```
**Output:**
```html
<p>bar</p>
```

## Link reference definitions

[CM-209] Example 209
**Input:**
```
[foo]: /url "title" ok
```
**Output:**
```html
<p>[foo]: /url &quot;title&quot; ok</p>
```

## Link reference definitions

[CM-210] Example 210
**Input:**
```
[foo]: /url
"title" ok
```
**Output:**
```html
<p>&quot;title&quot; ok</p>
```

## Link reference definitions

[CM-211] Example 211
**Input:**
```
    [foo]: /url "title"

[foo]
```
**Output:**
```html
<pre><code>[foo]: /url &quot;title&quot;
</code></pre>
<p>[foo]</p>
```

## Link reference definitions

[CM-212] Example 212
**Input:**
```
```
[foo]: /url
```

[foo]
```
**Output:**
```html
<pre><code>[foo]: /url
</code></pre>
<p>[foo]</p>
```

## Link reference definitions

[CM-213] Example 213
**Input:**
```
Foo
[bar]: /baz

[bar]
```
**Output:**
```html
<p>Foo
[bar]: /baz</p>
<p>[bar]</p>
```

## Link reference definitions

[CM-214] Example 214
**Input:**
```
# [Foo]
[foo]: /url
> bar
```
**Output:**
```html
<h1><a href="/url">Foo</a></h1>
<blockquote>
<p>bar</p>
</blockquote>
```

## Link reference definitions

[CM-215] Example 215
**Input:**
```
[foo]: /url
bar
===
[foo]
```
**Output:**
```html
<h1>bar</h1>
<p><a href="/url">foo</a></p>
```

## Link reference definitions

[CM-216] Example 216
**Input:**
```
[foo]: /url
===
[foo]
```
**Output:**
```html
<p>===
<a href="/url">foo</a></p>
```

## Link reference definitions

[CM-217] Example 217
**Input:**
```
[foo]: /foo-url "foo"
[bar]: /bar-url
  "bar"
[baz]: /baz-url

[foo],
[bar],
[baz]
```
**Output:**
```html
<p><a href="/foo-url" title="foo">foo</a>,
<a href="/bar-url" title="bar">bar</a>,
<a href="/baz-url">baz</a></p>
```

## Link reference definitions

[CM-218] Example 218
**Input:**
```
[foo]

> [foo]: /url
```
**Output:**
```html
<p><a href="/url">foo</a></p>
<blockquote>
</blockquote>
```

## Paragraphs

[CM-219] Example 219
**Input:**
```
aaa

bbb
```
**Output:**
```html
<p>aaa</p>
<p>bbb</p>
```

## Paragraphs

[CM-220] Example 220
**Input:**
```
aaa
bbb

ccc
ddd
```
**Output:**
```html
<p>aaa
bbb</p>
<p>ccc
ddd</p>
```

## Paragraphs

[CM-221] Example 221
**Input:**
```
aaa


bbb
```
**Output:**
```html
<p>aaa</p>
<p>bbb</p>
```

## Paragraphs

[CM-222] Example 222
**Input:**
```
  aaa
 bbb
```
**Output:**
```html
<p>aaa
bbb</p>
```

## Paragraphs

[CM-223] Example 223
**Input:**
```
aaa
             bbb
                                       ccc
```
**Output:**
```html
<p>aaa
bbb
ccc</p>
```

## Paragraphs

[CM-224] Example 224
**Input:**
```
   aaa
bbb
```
**Output:**
```html
<p>aaa
bbb</p>
```

## Paragraphs

[CM-225] Example 225
**Input:**
```
    aaa
bbb
```
**Output:**
```html
<pre><code>aaa
</code></pre>
<p>bbb</p>
```

## Paragraphs

[CM-226] Example 226
**Input:**
```
aaa     
bbb     
```
**Output:**
```html
<p>aaa<br />
bbb</p>
```

## Blank lines

[CM-227] Example 227
**Input:**
```
  

aaa
  

# aaa

  
```
**Output:**
```html
<p>aaa</p>
<h1>aaa</h1>
```

## Block quotes

[CM-228] Example 228
**Input:**
```
> # Foo
> bar
> baz
```
**Output:**
```html
<blockquote>
<h1>Foo</h1>
<p>bar
baz</p>
</blockquote>
```

## Block quotes

[CM-229] Example 229
**Input:**
```
># Foo
>bar
> baz
```
**Output:**
```html
<blockquote>
<h1>Foo</h1>
<p>bar
baz</p>
</blockquote>
```

## Block quotes

[CM-230] Example 230
**Input:**
```
   > # Foo
   > bar
 > baz
```
**Output:**
```html
<blockquote>
<h1>Foo</h1>
<p>bar
baz</p>
</blockquote>
```

## Block quotes

[CM-231] Example 231
**Input:**
```
    > # Foo
    > bar
    > baz
```
**Output:**
```html
<pre><code>&gt; # Foo
&gt; bar
&gt; baz
</code></pre>
```

## Block quotes

[CM-232] Example 232
**Input:**
```
> # Foo
> bar
baz
```
**Output:**
```html
<blockquote>
<h1>Foo</h1>
<p>bar
baz</p>
</blockquote>
```

## Block quotes

[CM-233] Example 233
**Input:**
```
> bar
baz
> foo
```
**Output:**
```html
<blockquote>
<p>bar
baz
foo</p>
</blockquote>
```

## Block quotes

[CM-234] Example 234
**Input:**
```
> foo
---
```
**Output:**
```html
<blockquote>
<p>foo</p>
</blockquote>
<hr />
```

## Block quotes

[CM-235] Example 235
**Input:**
```
> - foo
- bar
```
**Output:**
```html
<blockquote>
<ul>
<li>foo</li>
</ul>
</blockquote>
<ul>
<li>bar</li>
</ul>
```

## Block quotes

[CM-236] Example 236
**Input:**
```
>     foo
    bar
```
**Output:**
```html
<blockquote>
<pre><code>foo
</code></pre>
</blockquote>
<pre><code>bar
</code></pre>
```

## Block quotes

[CM-237] Example 237
**Input:**
```
> ```
foo
```
```
**Output:**
```html
<blockquote>
<pre><code></code></pre>
</blockquote>
<p>foo</p>
<pre><code></code></pre>
```

## Block quotes

[CM-238] Example 238
**Input:**
```
> foo
    - bar
```
**Output:**
```html
<blockquote>
<p>foo
- bar</p>
</blockquote>
```

## Block quotes

[CM-239] Example 239
**Input:**
```
>
```
**Output:**
```html
<blockquote>
</blockquote>
```

## Block quotes

[CM-240] Example 240
**Input:**
```
>
>  
> 
```
**Output:**
```html
<blockquote>
</blockquote>
```

## Block quotes

[CM-241] Example 241
**Input:**
```
>
> foo
>  
```
**Output:**
```html
<blockquote>
<p>foo</p>
</blockquote>
```

## Block quotes

[CM-242] Example 242
**Input:**
```
> foo

> bar
```
**Output:**
```html
<blockquote>
<p>foo</p>
</blockquote>
<blockquote>
<p>bar</p>
</blockquote>
```

## Block quotes

[CM-243] Example 243
**Input:**
```
> foo
> bar
```
**Output:**
```html
<blockquote>
<p>foo
bar</p>
</blockquote>
```

## Block quotes

[CM-244] Example 244
**Input:**
```
> foo
>
> bar
```
**Output:**
```html
<blockquote>
<p>foo</p>
<p>bar</p>
</blockquote>
```

## Block quotes

[CM-245] Example 245
**Input:**
```
foo
> bar
```
**Output:**
```html
<p>foo</p>
<blockquote>
<p>bar</p>
</blockquote>
```

## Block quotes

[CM-246] Example 246
**Input:**
```
> aaa
***
> bbb
```
**Output:**
```html
<blockquote>
<p>aaa</p>
</blockquote>
<hr />
<blockquote>
<p>bbb</p>
</blockquote>
```

## Block quotes

[CM-247] Example 247
**Input:**
```
> bar
baz
```
**Output:**
```html
<blockquote>
<p>bar
baz</p>
</blockquote>
```

## Block quotes

[CM-248] Example 248
**Input:**
```
> bar

baz
```
**Output:**
```html
<blockquote>
<p>bar</p>
</blockquote>
<p>baz</p>
```

## Block quotes

[CM-249] Example 249
**Input:**
```
> bar
>
baz
```
**Output:**
```html
<blockquote>
<p>bar</p>
</blockquote>
<p>baz</p>
```

## Block quotes

[CM-250] Example 250
**Input:**
```
> > > foo
bar
```
**Output:**
```html
<blockquote>
<blockquote>
<blockquote>
<p>foo
bar</p>
</blockquote>
</blockquote>
</blockquote>
```

## Block quotes

[CM-251] Example 251
**Input:**
```
>>> foo
> bar
>>baz
```
**Output:**
```html
<blockquote>
<blockquote>
<blockquote>
<p>foo
bar
baz</p>
</blockquote>
</blockquote>
</blockquote>
```

## Block quotes

[CM-252] Example 252
**Input:**
```
>     code

>    not code
```
**Output:**
```html
<blockquote>
<pre><code>code
</code></pre>
</blockquote>
<blockquote>
<p>not code</p>
</blockquote>
```

## List items

[CM-253] Example 253
**Input:**
```
A paragraph
with two lines.

    indented code

> A block quote.
```
**Output:**
```html
<p>A paragraph
with two lines.</p>
<pre><code>indented code
</code></pre>
<blockquote>
<p>A block quote.</p>
</blockquote>
```

## List items

[CM-254] Example 254
**Input:**
```
1.  A paragraph
    with two lines.

        indented code

    > A block quote.
```
**Output:**
```html
<ol>
<li>
<p>A paragraph
with two lines.</p>
<pre><code>indented code
</code></pre>
<blockquote>
<p>A block quote.</p>
</blockquote>
</li>
</ol>
```

## List items

[CM-255] Example 255
**Input:**
```
- one

 two
```
**Output:**
```html
<ul>
<li>one</li>
</ul>
<p>two</p>
```

## List items

[CM-256] Example 256
**Input:**
```
- one

  two
```
**Output:**
```html
<ul>
<li>
<p>one</p>
<p>two</p>
</li>
</ul>
```

## List items

[CM-257] Example 257
**Input:**
```
 -    one

     two
```
**Output:**
```html
<ul>
<li>one</li>
</ul>
<pre><code> two
</code></pre>
```

## List items

[CM-258] Example 258
**Input:**
```
 -    one

      two
```
**Output:**
```html
<ul>
<li>
<p>one</p>
<p>two</p>
</li>
</ul>
```

## List items

[CM-259] Example 259
**Input:**
```
   > > 1.  one
>>
>>     two
```
**Output:**
```html
<blockquote>
<blockquote>
<ol>
<li>
<p>one</p>
<p>two</p>
</li>
</ol>
</blockquote>
</blockquote>
```

## List items

[CM-260] Example 260
**Input:**
```
>>- one
>>
  >  > two
```
**Output:**
```html
<blockquote>
<blockquote>
<ul>
<li>one</li>
</ul>
<p>two</p>
</blockquote>
</blockquote>
```

## List items

[CM-261] Example 261
**Input:**
```
-one

2.two
```
**Output:**
```html
<p>-one</p>
<p>2.two</p>
```

## List items

[CM-262] Example 262
**Input:**
```
- foo


  bar
```
**Output:**
```html
<ul>
<li>
<p>foo</p>
<p>bar</p>
</li>
</ul>
```

## List items

[CM-263] Example 263
**Input:**
```
1.  foo

    ```
    bar
    ```

    baz

    > bam
```
**Output:**
```html
<ol>
<li>
<p>foo</p>
<pre><code>bar
</code></pre>
<p>baz</p>
<blockquote>
<p>bam</p>
</blockquote>
</li>
</ol>
```

## List items

[CM-264] Example 264
**Input:**
```
- Foo

      bar


      baz
```
**Output:**
```html
<ul>
<li>
<p>Foo</p>
<pre><code>bar


baz
</code></pre>
</li>
</ul>
```

## List items

[CM-265] Example 265
**Input:**
```
123456789. ok
```
**Output:**
```html
<ol start="123456789">
<li>ok</li>
</ol>
```

## List items

[CM-266] Example 266
**Input:**
```
1234567890. not ok
```
**Output:**
```html
<p>1234567890. not ok</p>
```

## List items

[CM-267] Example 267
**Input:**
```
0. ok
```
**Output:**
```html
<ol start="0">
<li>ok</li>
</ol>
```

## List items

[CM-268] Example 268
**Input:**
```
003. ok
```
**Output:**
```html
<ol start="3">
<li>ok</li>
</ol>
```

## List items

[CM-269] Example 269
**Input:**
```
-1. not ok
```
**Output:**
```html
<p>-1. not ok</p>
```

## List items

[CM-270] Example 270
**Input:**
```
- foo

      bar
```
**Output:**
```html
<ul>
<li>
<p>foo</p>
<pre><code>bar
</code></pre>
</li>
</ul>
```

## List items

[CM-271] Example 271
**Input:**
```
  10.  foo

           bar
```
**Output:**
```html
<ol start="10">
<li>
<p>foo</p>
<pre><code>bar
</code></pre>
</li>
</ol>
```

## List items

[CM-272] Example 272
**Input:**
```
    indented code

paragraph

    more code
```
**Output:**
```html
<pre><code>indented code
</code></pre>
<p>paragraph</p>
<pre><code>more code
</code></pre>
```

## List items

[CM-273] Example 273
**Input:**
```
1.     indented code

   paragraph

       more code
```
**Output:**
```html
<ol>
<li>
<pre><code>indented code
</code></pre>
<p>paragraph</p>
<pre><code>more code
</code></pre>
</li>
</ol>
```

## List items

[CM-274] Example 274
**Input:**
```
1.      indented code

   paragraph

       more code
```
**Output:**
```html
<ol>
<li>
<pre><code> indented code
</code></pre>
<p>paragraph</p>
<pre><code>more code
</code></pre>
</li>
</ol>
```

## List items

[CM-275] Example 275
**Input:**
```
   foo

bar
```
**Output:**
```html
<p>foo</p>
<p>bar</p>
```

## List items

[CM-276] Example 276
**Input:**
```
-    foo

  bar
```
**Output:**
```html
<ul>
<li>foo</li>
</ul>
<p>bar</p>
```

## List items

[CM-277] Example 277
**Input:**
```
-  foo

   bar
```
**Output:**
```html
<ul>
<li>
<p>foo</p>
<p>bar</p>
</li>
</ul>
```

## List items

[CM-278] Example 278
**Input:**
```
-
  foo
-
  ```
  bar
  ```
-
      baz
```
**Output:**
```html
<ul>
<li>foo</li>
<li>
<pre><code>bar
</code></pre>
</li>
<li>
<pre><code>baz
</code></pre>
</li>
</ul>
```

## List items

[CM-279] Example 279
**Input:**
```
-   
  foo
```
**Output:**
```html
<ul>
<li>foo</li>
</ul>
```

## List items

[CM-280] Example 280
**Input:**
```
-

  foo
```
**Output:**
```html
<ul>
<li></li>
</ul>
<p>foo</p>
```

## List items

[CM-281] Example 281
**Input:**
```
- foo
-
- bar
```
**Output:**
```html
<ul>
<li>foo</li>
<li></li>
<li>bar</li>
</ul>
```

## List items

[CM-282] Example 282
**Input:**
```
- foo
-   
- bar
```
**Output:**
```html
<ul>
<li>foo</li>
<li></li>
<li>bar</li>
</ul>
```

## List items

[CM-283] Example 283
**Input:**
```
1. foo
2.
3. bar
```
**Output:**
```html
<ol>
<li>foo</li>
<li></li>
<li>bar</li>
</ol>
```

## List items

[CM-284] Example 284
**Input:**
```
*
```
**Output:**
```html
<ul>
<li></li>
</ul>
```

## List items

[CM-285] Example 285
**Input:**
```
foo
*

foo
1.
```
**Output:**
```html
<p>foo
*</p>
<p>foo
1.</p>
```

## List items

[CM-286] Example 286
**Input:**
```
 1.  A paragraph
     with two lines.

         indented code

     > A block quote.
```
**Output:**
```html
<ol>
<li>
<p>A paragraph
with two lines.</p>
<pre><code>indented code
</code></pre>
<blockquote>
<p>A block quote.</p>
</blockquote>
</li>
</ol>
```

## List items

[CM-287] Example 287
**Input:**
```
  1.  A paragraph
      with two lines.

          indented code

      > A block quote.
```
**Output:**
```html
<ol>
<li>
<p>A paragraph
with two lines.</p>
<pre><code>indented code
</code></pre>
<blockquote>
<p>A block quote.</p>
</blockquote>
</li>
</ol>
```

## List items

[CM-288] Example 288
**Input:**
```
   1.  A paragraph
       with two lines.

           indented code

       > A block quote.
```
**Output:**
```html
<ol>
<li>
<p>A paragraph
with two lines.</p>
<pre><code>indented code
</code></pre>
<blockquote>
<p>A block quote.</p>
</blockquote>
</li>
</ol>
```

## List items

[CM-289] Example 289
**Input:**
```
    1.  A paragraph
        with two lines.

            indented code

        > A block quote.
```
**Output:**
```html
<pre><code>1.  A paragraph
    with two lines.

        indented code

    &gt; A block quote.
</code></pre>
```

## List items

[CM-290] Example 290
**Input:**
```
  1.  A paragraph
with two lines.

          indented code

      > A block quote.
```
**Output:**
```html
<ol>
<li>
<p>A paragraph
with two lines.</p>
<pre><code>indented code
</code></pre>
<blockquote>
<p>A block quote.</p>
</blockquote>
</li>
</ol>
```

## List items

[CM-291] Example 291
**Input:**
```
  1.  A paragraph
    with two lines.
```
**Output:**
```html
<ol>
<li>A paragraph
with two lines.</li>
</ol>
```

## List items

[CM-292] Example 292
**Input:**
```
> 1. > Blockquote
continued here.
```
**Output:**
```html
<blockquote>
<ol>
<li>
<blockquote>
<p>Blockquote
continued here.</p>
</blockquote>
</li>
</ol>
</blockquote>
```

## List items

[CM-293] Example 293
**Input:**
```
> 1. > Blockquote
> continued here.
```
**Output:**
```html
<blockquote>
<ol>
<li>
<blockquote>
<p>Blockquote
continued here.</p>
</blockquote>
</li>
</ol>
</blockquote>
```

## List items

[CM-294] Example 294
**Input:**
```
- foo
  - bar
    - baz
      - boo
```
**Output:**
```html
<ul>
<li>foo
<ul>
<li>bar
<ul>
<li>baz
<ul>
<li>boo</li>
</ul>
</li>
</ul>
</li>
</ul>
</li>
</ul>
```

## List items

[CM-295] Example 295
**Input:**
```
- foo
 - bar
  - baz
   - boo
```
**Output:**
```html
<ul>
<li>foo</li>
<li>bar</li>
<li>baz</li>
<li>boo</li>
</ul>
```

## List items

[CM-296] Example 296
**Input:**
```
10) foo
    - bar
```
**Output:**
```html
<ol start="10">
<li>foo
<ul>
<li>bar</li>
</ul>
</li>
</ol>
```

## List items

[CM-297] Example 297
**Input:**
```
10) foo
   - bar
```
**Output:**
```html
<ol start="10">
<li>foo</li>
</ol>
<ul>
<li>bar</li>
</ul>
```

## List items

[CM-298] Example 298
**Input:**
```
- - foo
```
**Output:**
```html
<ul>
<li>
<ul>
<li>foo</li>
</ul>
</li>
</ul>
```

## List items

[CM-299] Example 299
**Input:**
```
1. - 2. foo
```
**Output:**
```html
<ol>
<li>
<ul>
<li>
<ol start="2">
<li>foo</li>
</ol>
</li>
</ul>
</li>
</ol>
```

## List items

[CM-300] Example 300
**Input:**
```
- # Foo
- Bar
  ---
  baz
```
**Output:**
```html
<ul>
<li>
<h1>Foo</h1>
</li>
<li>
<h2>Bar</h2>
baz</li>
</ul>
```

## Lists

[CM-301] Example 301
**Input:**
```
- foo
- bar
+ baz
```
**Output:**
```html
<ul>
<li>foo</li>
<li>bar</li>
</ul>
<ul>
<li>baz</li>
</ul>
```

## Lists

[CM-302] Example 302
**Input:**
```
1. foo
2. bar
3) baz
```
**Output:**
```html
<ol>
<li>foo</li>
<li>bar</li>
</ol>
<ol start="3">
<li>baz</li>
</ol>
```

## Lists

[CM-303] Example 303
**Input:**
```
Foo
- bar
- baz
```
**Output:**
```html
<p>Foo</p>
<ul>
<li>bar</li>
<li>baz</li>
</ul>
```

## Lists

[CM-304] Example 304
**Input:**
```
The number of windows in my house is
14.  The number of doors is 6.
```
**Output:**
```html
<p>The number of windows in my house is
14.  The number of doors is 6.</p>
```

## Lists

[CM-305] Example 305
**Input:**
```
The number of windows in my house is
1.  The number of doors is 6.
```
**Output:**
```html
<p>The number of windows in my house is</p>
<ol>
<li>The number of doors is 6.</li>
</ol>
```

## Lists

[CM-306] Example 306
**Input:**
```
- foo

- bar


- baz
```
**Output:**
```html
<ul>
<li>
<p>foo</p>
</li>
<li>
<p>bar</p>
</li>
<li>
<p>baz</p>
</li>
</ul>
```

## Lists

[CM-307] Example 307
**Input:**
```
- foo
  - bar
    - baz


      bim
```
**Output:**
```html
<ul>
<li>foo
<ul>
<li>bar
<ul>
<li>
<p>baz</p>
<p>bim</p>
</li>
</ul>
</li>
</ul>
</li>
</ul>
```

## Lists

[CM-308] Example 308
**Input:**
```
- foo
- bar

<!-- -->

- baz
- bim
```
**Output:**
```html
<ul>
<li>foo</li>
<li>bar</li>
</ul>
<!-- -->
<ul>
<li>baz</li>
<li>bim</li>
</ul>
```

## Lists

[CM-309] Example 309
**Input:**
```
-   foo

    notcode

-   foo

<!-- -->

    code
```
**Output:**
```html
<ul>
<li>
<p>foo</p>
<p>notcode</p>
</li>
<li>
<p>foo</p>
</li>
</ul>
<!-- -->
<pre><code>code
</code></pre>
```

## Lists

[CM-310] Example 310
**Input:**
```
- a
 - b
  - c
   - d
  - e
 - f
- g
```
**Output:**
```html
<ul>
<li>a</li>
<li>b</li>
<li>c</li>
<li>d</li>
<li>e</li>
<li>f</li>
<li>g</li>
</ul>
```

## Lists

[CM-311] Example 311
**Input:**
```
1. a

  2. b

   3. c
```
**Output:**
```html
<ol>
<li>
<p>a</p>
</li>
<li>
<p>b</p>
</li>
<li>
<p>c</p>
</li>
</ol>
```

## Lists

[CM-312] Example 312
**Input:**
```
- a
 - b
  - c
   - d
    - e
```
**Output:**
```html
<ul>
<li>a</li>
<li>b</li>
<li>c</li>
<li>d
- e</li>
</ul>
```

## Lists

[CM-313] Example 313
**Input:**
```
1. a

  2. b

    3. c
```
**Output:**
```html
<ol>
<li>
<p>a</p>
</li>
<li>
<p>b</p>
</li>
</ol>
<pre><code>3. c
</code></pre>
```

## Lists

[CM-314] Example 314
**Input:**
```
- a
- b

- c
```
**Output:**
```html
<ul>
<li>
<p>a</p>
</li>
<li>
<p>b</p>
</li>
<li>
<p>c</p>
</li>
</ul>
```

## Lists

[CM-315] Example 315
**Input:**
```
* a
*

* c
```
**Output:**
```html
<ul>
<li>
<p>a</p>
</li>
<li></li>
<li>
<p>c</p>
</li>
</ul>
```

## Lists

[CM-316] Example 316
**Input:**
```
- a
- b

  c
- d
```
**Output:**
```html
<ul>
<li>
<p>a</p>
</li>
<li>
<p>b</p>
<p>c</p>
</li>
<li>
<p>d</p>
</li>
</ul>
```

## Lists

[CM-317] Example 317
**Input:**
```
- a
- b

  [ref]: /url
- d
```
**Output:**
```html
<ul>
<li>
<p>a</p>
</li>
<li>
<p>b</p>
</li>
<li>
<p>d</p>
</li>
</ul>
```

## Lists

[CM-318] Example 318
**Input:**
```
- a
- ```
  b


  ```
- c
```
**Output:**
```html
<ul>
<li>a</li>
<li>
<pre><code>b


</code></pre>
</li>
<li>c</li>
</ul>
```

## Lists

[CM-319] Example 319
**Input:**
```
- a
  - b

    c
- d
```
**Output:**
```html
<ul>
<li>a
<ul>
<li>
<p>b</p>
<p>c</p>
</li>
</ul>
</li>
<li>d</li>
</ul>
```

## Lists

[CM-320] Example 320
**Input:**
```
* a
  > b
  >
* c
```
**Output:**
```html
<ul>
<li>a
<blockquote>
<p>b</p>
</blockquote>
</li>
<li>c</li>
</ul>
```

## Lists

[CM-321] Example 321
**Input:**
```
- a
  > b
  ```
  c
  ```
- d
```
**Output:**
```html
<ul>
<li>a
<blockquote>
<p>b</p>
</blockquote>
<pre><code>c
</code></pre>
</li>
<li>d</li>
</ul>
```

## Lists

[CM-322] Example 322
**Input:**
```
- a
```
**Output:**
```html
<ul>
<li>a</li>
</ul>
```

## Lists

[CM-323] Example 323
**Input:**
```
- a
  - b
```
**Output:**
```html
<ul>
<li>a
<ul>
<li>b</li>
</ul>
</li>
</ul>
```

## Lists

[CM-324] Example 324
**Input:**
```
1. ```
   foo
   ```

   bar
```
**Output:**
```html
<ol>
<li>
<pre><code>foo
</code></pre>
<p>bar</p>
</li>
</ol>
```

## Lists

[CM-325] Example 325
**Input:**
```
* foo
  * bar

  baz
```
**Output:**
```html
<ul>
<li>
<p>foo</p>
<ul>
<li>bar</li>
</ul>
<p>baz</p>
</li>
</ul>
```

## Lists

[CM-326] Example 326
**Input:**
```
- a
  - b
  - c

- d
  - e
  - f
```
**Output:**
```html
<ul>
<li>
<p>a</p>
<ul>
<li>b</li>
<li>c</li>
</ul>
</li>
<li>
<p>d</p>
<ul>
<li>e</li>
<li>f</li>
</ul>
</li>
</ul>
```

## Inlines

[CM-327] Example 327
**Input:**
```
`hi`lo`
```
**Output:**
```html
<p><code>hi</code>lo`</p>
```

## Code spans

[CM-328] Example 328
**Input:**
```
`foo`
```
**Output:**
```html
<p><code>foo</code></p>
```

## Code spans

[CM-329] Example 329
**Input:**
```
`` foo ` bar ``
```
**Output:**
```html
<p><code>foo ` bar</code></p>
```

## Code spans

[CM-330] Example 330
**Input:**
```
` `` `
```
**Output:**
```html
<p><code>``</code></p>
```

## Code spans

[CM-331] Example 331
**Input:**
```
`  ``  `
```
**Output:**
```html
<p><code> `` </code></p>
```

## Code spans

[CM-332] Example 332
**Input:**
```
` a`
```
**Output:**
```html
<p><code> a</code></p>
```

## Code spans

[CM-333] Example 333
**Input:**
```
` b `
```
**Output:**
```html
<p><code> b </code></p>
```

## Code spans

[CM-334] Example 334
**Input:**
```
` `
`  `
```
**Output:**
```html
<p><code> </code>
<code>  </code></p>
```

## Code spans

[CM-335] Example 335
**Input:**
```
``
foo
bar  
baz
``
```
**Output:**
```html
<p><code>foo bar   baz</code></p>
```

## Code spans

[CM-336] Example 336
**Input:**
```
``
foo 
``
```
**Output:**
```html
<p><code>foo </code></p>
```

## Code spans

[CM-337] Example 337
**Input:**
```
`foo   bar 
baz`
```
**Output:**
```html
<p><code>foo   bar  baz</code></p>
```

## Code spans

[CM-338] Example 338
**Input:**
```
`foo\`bar`
```
**Output:**
```html
<p><code>foo\</code>bar`</p>
```

## Code spans

[CM-339] Example 339
**Input:**
```
``foo`bar``
```
**Output:**
```html
<p><code>foo`bar</code></p>
```

## Code spans

[CM-340] Example 340
**Input:**
```
` foo `` bar `
```
**Output:**
```html
<p><code>foo `` bar</code></p>
```

## Code spans

[CM-341] Example 341
**Input:**
```
*foo`*`
```
**Output:**
```html
<p>*foo<code>*</code></p>
```

## Code spans

[CM-342] Example 342
**Input:**
```
[not a `link](/foo`)
```
**Output:**
```html
<p>[not a <code>link](/foo</code>)</p>
```

## Code spans

[CM-343] Example 343
**Input:**
```
`<a href="`">`
```
**Output:**
```html
<p><code>&lt;a href=&quot;</code>&quot;&gt;`</p>
```

## Code spans

[CM-344] Example 344
**Input:**
```
<a href="`">`
```
**Output:**
```html
<p><a href="`">`</p>
```

## Code spans

[CM-345] Example 345
**Input:**
```
`<https://foo.bar.`baz>`
```
**Output:**
```html
<p><code>&lt;https://foo.bar.</code>baz&gt;`</p>
```

## Code spans

[CM-346] Example 346
**Input:**
```
<https://foo.bar.`baz>`
```
**Output:**
```html
<p><a href="https://foo.bar.%60baz">https://foo.bar.`baz</a>`</p>
```

## Code spans

[CM-347] Example 347
**Input:**
```
```foo``
```
**Output:**
```html
<p>```foo``</p>
```

## Code spans

[CM-348] Example 348
**Input:**
```
`foo
```
**Output:**
```html
<p>`foo</p>
```

## Code spans

[CM-349] Example 349
**Input:**
```
`foo``bar``
```
**Output:**
```html
<p>`foo<code>bar</code></p>
```

## Emphasis and strong emphasis

[CM-350] Example 350
**Input:**
```
*foo bar*
```
**Output:**
```html
<p><em>foo bar</em></p>
```

## Emphasis and strong emphasis

[CM-351] Example 351
**Input:**
```
a * foo bar*
```
**Output:**
```html
<p>a * foo bar*</p>
```

## Emphasis and strong emphasis

[CM-352] Example 352
**Input:**
```
a*"foo"*
```
**Output:**
```html
<p>a*&quot;foo&quot;*</p>
```

## Emphasis and strong emphasis

[CM-353] Example 353
**Input:**
```
* a *
```
**Output:**
```html
<p>* a *</p>
```

## Emphasis and strong emphasis

[CM-354] Example 354
**Input:**
```
*$*alpha.

*£*bravo.

*€*charlie.
```
**Output:**
```html
<p>*$*alpha.</p>
<p>*£*bravo.</p>
<p>*€*charlie.</p>
```

## Emphasis and strong emphasis

[CM-355] Example 355
**Input:**
```
foo*bar*
```
**Output:**
```html
<p>foo<em>bar</em></p>
```

## Emphasis and strong emphasis

[CM-356] Example 356
**Input:**
```
5*6*78
```
**Output:**
```html
<p>5<em>6</em>78</p>
```

## Emphasis and strong emphasis

[CM-357] Example 357
**Input:**
```
_foo bar_
```
**Output:**
```html
<p><em>foo bar</em></p>
```

## Emphasis and strong emphasis

[CM-358] Example 358
**Input:**
```
_ foo bar_
```
**Output:**
```html
<p>_ foo bar_</p>
```

## Emphasis and strong emphasis

[CM-359] Example 359
**Input:**
```
a_"foo"_
```
**Output:**
```html
<p>a_&quot;foo&quot;_</p>
```

## Emphasis and strong emphasis

[CM-360] Example 360
**Input:**
```
foo_bar_
```
**Output:**
```html
<p>foo_bar_</p>
```

## Emphasis and strong emphasis

[CM-361] Example 361
**Input:**
```
5_6_78
```
**Output:**
```html
<p>5_6_78</p>
```

## Emphasis and strong emphasis

[CM-362] Example 362
**Input:**
```
пристаням_стремятся_
```
**Output:**
```html
<p>пристаням_стремятся_</p>
```

## Emphasis and strong emphasis

[CM-363] Example 363
**Input:**
```
aa_"bb"_cc
```
**Output:**
```html
<p>aa_&quot;bb&quot;_cc</p>
```

## Emphasis and strong emphasis

[CM-364] Example 364
**Input:**
```
foo-_(bar)_
```
**Output:**
```html
<p>foo-<em>(bar)</em></p>
```

## Emphasis and strong emphasis

[CM-365] Example 365
**Input:**
```
_foo*
```
**Output:**
```html
<p>_foo*</p>
```

## Emphasis and strong emphasis

[CM-366] Example 366
**Input:**
```
*foo bar *
```
**Output:**
```html
<p>*foo bar *</p>
```

## Emphasis and strong emphasis

[CM-367] Example 367
**Input:**
```
*foo bar
*
```
**Output:**
```html
<p>*foo bar
*</p>
```

## Emphasis and strong emphasis

[CM-368] Example 368
**Input:**
```
*(*foo)
```
**Output:**
```html
<p>*(*foo)</p>
```

## Emphasis and strong emphasis

[CM-369] Example 369
**Input:**
```
*(*foo*)*
```
**Output:**
```html
<p><em>(<em>foo</em>)</em></p>
```

## Emphasis and strong emphasis

[CM-370] Example 370
**Input:**
```
*foo*bar
```
**Output:**
```html
<p><em>foo</em>bar</p>
```

## Emphasis and strong emphasis

[CM-371] Example 371
**Input:**
```
_foo bar _
```
**Output:**
```html
<p>_foo bar _</p>
```

## Emphasis and strong emphasis

[CM-372] Example 372
**Input:**
```
_(_foo)
```
**Output:**
```html
<p>_(_foo)</p>
```

## Emphasis and strong emphasis

[CM-373] Example 373
**Input:**
```
_(_foo_)_
```
**Output:**
```html
<p><em>(<em>foo</em>)</em></p>
```

## Emphasis and strong emphasis

[CM-374] Example 374
**Input:**
```
_foo_bar
```
**Output:**
```html
<p>_foo_bar</p>
```

## Emphasis and strong emphasis

[CM-375] Example 375
**Input:**
```
_пристаням_стремятся
```
**Output:**
```html
<p>_пристаням_стремятся</p>
```

## Emphasis and strong emphasis

[CM-376] Example 376
**Input:**
```
_foo_bar_baz_
```
**Output:**
```html
<p><em>foo_bar_baz</em></p>
```

## Emphasis and strong emphasis

[CM-377] Example 377
**Input:**
```
_(bar)_.
```
**Output:**
```html
<p><em>(bar)</em>.</p>
```

## Emphasis and strong emphasis

[CM-378] Example 378
**Input:**
```
**foo bar**
```
**Output:**
```html
<p><strong>foo bar</strong></p>
```

## Emphasis and strong emphasis

[CM-379] Example 379
**Input:**
```
** foo bar**
```
**Output:**
```html
<p>** foo bar**</p>
```

## Emphasis and strong emphasis

[CM-380] Example 380
**Input:**
```
a**"foo"**
```
**Output:**
```html
<p>a**&quot;foo&quot;**</p>
```

## Emphasis and strong emphasis

[CM-381] Example 381
**Input:**
```
foo**bar**
```
**Output:**
```html
<p>foo<strong>bar</strong></p>
```

## Emphasis and strong emphasis

[CM-382] Example 382
**Input:**
```
__foo bar__
```
**Output:**
```html
<p><strong>foo bar</strong></p>
```

## Emphasis and strong emphasis

[CM-383] Example 383
**Input:**
```
__ foo bar__
```
**Output:**
```html
<p>__ foo bar__</p>
```

## Emphasis and strong emphasis

[CM-384] Example 384
**Input:**
```
__
foo bar__
```
**Output:**
```html
<p>__
foo bar__</p>
```

## Emphasis and strong emphasis

[CM-385] Example 385
**Input:**
```
a__"foo"__
```
**Output:**
```html
<p>a__&quot;foo&quot;__</p>
```

## Emphasis and strong emphasis

[CM-386] Example 386
**Input:**
```
foo__bar__
```
**Output:**
```html
<p>foo__bar__</p>
```

## Emphasis and strong emphasis

[CM-387] Example 387
**Input:**
```
5__6__78
```
**Output:**
```html
<p>5__6__78</p>
```

## Emphasis and strong emphasis

[CM-388] Example 388
**Input:**
```
пристаням__стремятся__
```
**Output:**
```html
<p>пристаням__стремятся__</p>
```

## Emphasis and strong emphasis

[CM-389] Example 389
**Input:**
```
__foo, __bar__, baz__
```
**Output:**
```html
<p><strong>foo, <strong>bar</strong>, baz</strong></p>
```

## Emphasis and strong emphasis

[CM-390] Example 390
**Input:**
```
foo-__(bar)__
```
**Output:**
```html
<p>foo-<strong>(bar)</strong></p>
```

## Emphasis and strong emphasis

[CM-391] Example 391
**Input:**
```
**foo bar **
```
**Output:**
```html
<p>**foo bar **</p>
```

## Emphasis and strong emphasis

[CM-392] Example 392
**Input:**
```
**(**foo)
```
**Output:**
```html
<p>**(**foo)</p>
```

## Emphasis and strong emphasis

[CM-393] Example 393
**Input:**
```
*(**foo**)*
```
**Output:**
```html
<p><em>(<strong>foo</strong>)</em></p>
```

## Emphasis and strong emphasis

[CM-394] Example 394
**Input:**
```
**Gomphocarpus (*Gomphocarpus physocarpus*, syn.
*Asclepias physocarpa*)**
```
**Output:**
```html
<p><strong>Gomphocarpus (<em>Gomphocarpus physocarpus</em>, syn.
<em>Asclepias physocarpa</em>)</strong></p>
```

## Emphasis and strong emphasis

[CM-395] Example 395
**Input:**
```
**foo "*bar*" foo**
```
**Output:**
```html
<p><strong>foo &quot;<em>bar</em>&quot; foo</strong></p>
```

## Emphasis and strong emphasis

[CM-396] Example 396
**Input:**
```
**foo**bar
```
**Output:**
```html
<p><strong>foo</strong>bar</p>
```

## Emphasis and strong emphasis

[CM-397] Example 397
**Input:**
```
__foo bar __
```
**Output:**
```html
<p>__foo bar __</p>
```

## Emphasis and strong emphasis

[CM-398] Example 398
**Input:**
```
__(__foo)
```
**Output:**
```html
<p>__(__foo)</p>
```

## Emphasis and strong emphasis

[CM-399] Example 399
**Input:**
```
_(__foo__)_
```
**Output:**
```html
<p><em>(<strong>foo</strong>)</em></p>
```

## Emphasis and strong emphasis

[CM-400] Example 400
**Input:**
```
__foo__bar
```
**Output:**
```html
<p>__foo__bar</p>
```

## Emphasis and strong emphasis

[CM-401] Example 401
**Input:**
```
__пристаням__стремятся
```
**Output:**
```html
<p>__пристаням__стремятся</p>
```

## Emphasis and strong emphasis

[CM-402] Example 402
**Input:**
```
__foo__bar__baz__
```
**Output:**
```html
<p><strong>foo__bar__baz</strong></p>
```

## Emphasis and strong emphasis

[CM-403] Example 403
**Input:**
```
__(bar)__.
```
**Output:**
```html
<p><strong>(bar)</strong>.</p>
```

## Emphasis and strong emphasis

[CM-404] Example 404
**Input:**
```
*foo [bar](/url)*
```
**Output:**
```html
<p><em>foo <a href="/url">bar</a></em></p>
```

## Emphasis and strong emphasis

[CM-405] Example 405
**Input:**
```
*foo
bar*
```
**Output:**
```html
<p><em>foo
bar</em></p>
```

## Emphasis and strong emphasis

[CM-406] Example 406
**Input:**
```
_foo __bar__ baz_
```
**Output:**
```html
<p><em>foo <strong>bar</strong> baz</em></p>
```

## Emphasis and strong emphasis

[CM-407] Example 407
**Input:**
```
_foo _bar_ baz_
```
**Output:**
```html
<p><em>foo <em>bar</em> baz</em></p>
```

## Emphasis and strong emphasis

[CM-408] Example 408
**Input:**
```
__foo_ bar_
```
**Output:**
```html
<p><em><em>foo</em> bar</em></p>
```

## Emphasis and strong emphasis

[CM-409] Example 409
**Input:**
```
*foo *bar**
```
**Output:**
```html
<p><em>foo <em>bar</em></em></p>
```

## Emphasis and strong emphasis

[CM-410] Example 410
**Input:**
```
*foo **bar** baz*
```
**Output:**
```html
<p><em>foo <strong>bar</strong> baz</em></p>
```

## Emphasis and strong emphasis

[CM-411] Example 411
**Input:**
```
*foo**bar**baz*
```
**Output:**
```html
<p><em>foo<strong>bar</strong>baz</em></p>
```

## Emphasis and strong emphasis

[CM-412] Example 412
**Input:**
```
*foo**bar*
```
**Output:**
```html
<p><em>foo**bar</em></p>
```

## Emphasis and strong emphasis

[CM-413] Example 413
**Input:**
```
***foo** bar*
```
**Output:**
```html
<p><em><strong>foo</strong> bar</em></p>
```

## Emphasis and strong emphasis

[CM-414] Example 414
**Input:**
```
*foo **bar***
```
**Output:**
```html
<p><em>foo <strong>bar</strong></em></p>
```

## Emphasis and strong emphasis

[CM-415] Example 415
**Input:**
```
*foo**bar***
```
**Output:**
```html
<p><em>foo<strong>bar</strong></em></p>
```

## Emphasis and strong emphasis

[CM-416] Example 416
**Input:**
```
foo***bar***baz
```
**Output:**
```html
<p>foo<em><strong>bar</strong></em>baz</p>
```

## Emphasis and strong emphasis

[CM-417] Example 417
**Input:**
```
foo******bar*********baz
```
**Output:**
```html
<p>foo<strong><strong><strong>bar</strong></strong></strong>***baz</p>
```

## Emphasis and strong emphasis

[CM-418] Example 418
**Input:**
```
*foo **bar *baz* bim** bop*
```
**Output:**
```html
<p><em>foo <strong>bar <em>baz</em> bim</strong> bop</em></p>
```

## Emphasis and strong emphasis

[CM-419] Example 419
**Input:**
```
*foo [*bar*](/url)*
```
**Output:**
```html
<p><em>foo <a href="/url"><em>bar</em></a></em></p>
```

## Emphasis and strong emphasis

[CM-420] Example 420
**Input:**
```
** is not an empty emphasis
```
**Output:**
```html
<p>** is not an empty emphasis</p>
```

## Emphasis and strong emphasis

[CM-421] Example 421
**Input:**
```
**** is not an empty strong emphasis
```
**Output:**
```html
<p>**** is not an empty strong emphasis</p>
```

## Emphasis and strong emphasis

[CM-422] Example 422
**Input:**
```
**foo [bar](/url)**
```
**Output:**
```html
<p><strong>foo <a href="/url">bar</a></strong></p>
```

## Emphasis and strong emphasis

[CM-423] Example 423
**Input:**
```
**foo
bar**
```
**Output:**
```html
<p><strong>foo
bar</strong></p>
```

## Emphasis and strong emphasis

[CM-424] Example 424
**Input:**
```
__foo _bar_ baz__
```
**Output:**
```html
<p><strong>foo <em>bar</em> baz</strong></p>
```

## Emphasis and strong emphasis

[CM-425] Example 425
**Input:**
```
__foo __bar__ baz__
```
**Output:**
```html
<p><strong>foo <strong>bar</strong> baz</strong></p>
```

## Emphasis and strong emphasis

[CM-426] Example 426
**Input:**
```
____foo__ bar__
```
**Output:**
```html
<p><strong><strong>foo</strong> bar</strong></p>
```

## Emphasis and strong emphasis

[CM-427] Example 427
**Input:**
```
**foo **bar****
```
**Output:**
```html
<p><strong>foo <strong>bar</strong></strong></p>
```

## Emphasis and strong emphasis

[CM-428] Example 428
**Input:**
```
**foo *bar* baz**
```
**Output:**
```html
<p><strong>foo <em>bar</em> baz</strong></p>
```

## Emphasis and strong emphasis

[CM-429] Example 429
**Input:**
```
**foo*bar*baz**
```
**Output:**
```html
<p><strong>foo<em>bar</em>baz</strong></p>
```

## Emphasis and strong emphasis

[CM-430] Example 430
**Input:**
```
***foo* bar**
```
**Output:**
```html
<p><strong><em>foo</em> bar</strong></p>
```

## Emphasis and strong emphasis

[CM-431] Example 431
**Input:**
```
**foo *bar***
```
**Output:**
```html
<p><strong>foo <em>bar</em></strong></p>
```

## Emphasis and strong emphasis

[CM-432] Example 432
**Input:**
```
**foo *bar **baz**
bim* bop**
```
**Output:**
```html
<p><strong>foo <em>bar <strong>baz</strong>
bim</em> bop</strong></p>
```

## Emphasis and strong emphasis

[CM-433] Example 433
**Input:**
```
**foo [*bar*](/url)**
```
**Output:**
```html
<p><strong>foo <a href="/url"><em>bar</em></a></strong></p>
```

## Emphasis and strong emphasis

[CM-434] Example 434
**Input:**
```
__ is not an empty emphasis
```
**Output:**
```html
<p>__ is not an empty emphasis</p>
```

## Emphasis and strong emphasis

[CM-435] Example 435
**Input:**
```
____ is not an empty strong emphasis
```
**Output:**
```html
<p>____ is not an empty strong emphasis</p>
```

## Emphasis and strong emphasis

[CM-436] Example 436
**Input:**
```
foo ***
```
**Output:**
```html
<p>foo ***</p>
```

## Emphasis and strong emphasis

[CM-437] Example 437
**Input:**
```
foo *\**
```
**Output:**
```html
<p>foo <em>*</em></p>
```

## Emphasis and strong emphasis

[CM-438] Example 438
**Input:**
```
foo *_*
```
**Output:**
```html
<p>foo <em>_</em></p>
```

## Emphasis and strong emphasis

[CM-439] Example 439
**Input:**
```
foo *****
```
**Output:**
```html
<p>foo *****</p>
```

## Emphasis and strong emphasis

[CM-440] Example 440
**Input:**
```
foo **\***
```
**Output:**
```html
<p>foo <strong>*</strong></p>
```

## Emphasis and strong emphasis

[CM-441] Example 441
**Input:**
```
foo **_**
```
**Output:**
```html
<p>foo <strong>_</strong></p>
```

## Emphasis and strong emphasis

[CM-442] Example 442
**Input:**
```
**foo*
```
**Output:**
```html
<p>*<em>foo</em></p>
```

## Emphasis and strong emphasis

[CM-443] Example 443
**Input:**
```
*foo**
```
**Output:**
```html
<p><em>foo</em>*</p>
```

## Emphasis and strong emphasis

[CM-444] Example 444
**Input:**
```
***foo**
```
**Output:**
```html
<p>*<strong>foo</strong></p>
```

## Emphasis and strong emphasis

[CM-445] Example 445
**Input:**
```
****foo*
```
**Output:**
```html
<p>***<em>foo</em></p>
```

## Emphasis and strong emphasis

[CM-446] Example 446
**Input:**
```
**foo***
```
**Output:**
```html
<p><strong>foo</strong>*</p>
```

## Emphasis and strong emphasis

[CM-447] Example 447
**Input:**
```
*foo****
```
**Output:**
```html
<p><em>foo</em>***</p>
```

## Emphasis and strong emphasis

[CM-448] Example 448
**Input:**
```
foo ___
```
**Output:**
```html
<p>foo ___</p>
```

## Emphasis and strong emphasis

[CM-449] Example 449
**Input:**
```
foo _\__
```
**Output:**
```html
<p>foo <em>_</em></p>
```

## Emphasis and strong emphasis

[CM-450] Example 450
**Input:**
```
foo _*_
```
**Output:**
```html
<p>foo <em>*</em></p>
```

## Emphasis and strong emphasis

[CM-451] Example 451
**Input:**
```
foo _____
```
**Output:**
```html
<p>foo _____</p>
```

## Emphasis and strong emphasis

[CM-452] Example 452
**Input:**
```
foo __\___
```
**Output:**
```html
<p>foo <strong>_</strong></p>
```

## Emphasis and strong emphasis

[CM-453] Example 453
**Input:**
```
foo __*__
```
**Output:**
```html
<p>foo <strong>*</strong></p>
```

## Emphasis and strong emphasis

[CM-454] Example 454
**Input:**
```
__foo_
```
**Output:**
```html
<p>_<em>foo</em></p>
```

## Emphasis and strong emphasis

[CM-455] Example 455
**Input:**
```
_foo__
```
**Output:**
```html
<p><em>foo</em>_</p>
```

## Emphasis and strong emphasis

[CM-456] Example 456
**Input:**
```
___foo__
```
**Output:**
```html
<p>_<strong>foo</strong></p>
```

## Emphasis and strong emphasis

[CM-457] Example 457
**Input:**
```
____foo_
```
**Output:**
```html
<p>___<em>foo</em></p>
```

## Emphasis and strong emphasis

[CM-458] Example 458
**Input:**
```
__foo___
```
**Output:**
```html
<p><strong>foo</strong>_</p>
```

## Emphasis and strong emphasis

[CM-459] Example 459
**Input:**
```
_foo____
```
**Output:**
```html
<p><em>foo</em>___</p>
```

## Emphasis and strong emphasis

[CM-460] Example 460
**Input:**
```
**foo**
```
**Output:**
```html
<p><strong>foo</strong></p>
```

## Emphasis and strong emphasis

[CM-461] Example 461
**Input:**
```
*_foo_*
```
**Output:**
```html
<p><em><em>foo</em></em></p>
```

## Emphasis and strong emphasis

[CM-462] Example 462
**Input:**
```
__foo__
```
**Output:**
```html
<p><strong>foo</strong></p>
```

## Emphasis and strong emphasis

[CM-463] Example 463
**Input:**
```
_*foo*_
```
**Output:**
```html
<p><em><em>foo</em></em></p>
```

## Emphasis and strong emphasis

[CM-464] Example 464
**Input:**
```
****foo****
```
**Output:**
```html
<p><strong><strong>foo</strong></strong></p>
```

## Emphasis and strong emphasis

[CM-465] Example 465
**Input:**
```
____foo____
```
**Output:**
```html
<p><strong><strong>foo</strong></strong></p>
```

## Emphasis and strong emphasis

[CM-466] Example 466
**Input:**
```
******foo******
```
**Output:**
```html
<p><strong><strong><strong>foo</strong></strong></strong></p>
```

## Emphasis and strong emphasis

[CM-467] Example 467
**Input:**
```
***foo***
```
**Output:**
```html
<p><em><strong>foo</strong></em></p>
```

## Emphasis and strong emphasis

[CM-468] Example 468
**Input:**
```
_____foo_____
```
**Output:**
```html
<p><em><strong><strong>foo</strong></strong></em></p>
```

## Emphasis and strong emphasis

[CM-469] Example 469
**Input:**
```
*foo _bar* baz_
```
**Output:**
```html
<p><em>foo _bar</em> baz_</p>
```

## Emphasis and strong emphasis

[CM-470] Example 470
**Input:**
```
*foo __bar *baz bim__ bam*
```
**Output:**
```html
<p><em>foo <strong>bar *baz bim</strong> bam</em></p>
```

## Emphasis and strong emphasis

[CM-471] Example 471
**Input:**
```
**foo **bar baz**
```
**Output:**
```html
<p>**foo <strong>bar baz</strong></p>
```

## Emphasis and strong emphasis

[CM-472] Example 472
**Input:**
```
*foo *bar baz*
```
**Output:**
```html
<p>*foo <em>bar baz</em></p>
```

## Emphasis and strong emphasis

[CM-473] Example 473
**Input:**
```
*[bar*](/url)
```
**Output:**
```html
<p>*<a href="/url">bar*</a></p>
```

## Emphasis and strong emphasis

[CM-474] Example 474
**Input:**
```
_foo [bar_](/url)
```
**Output:**
```html
<p>_foo <a href="/url">bar_</a></p>
```

## Emphasis and strong emphasis

[CM-475] Example 475
**Input:**
```
*<img src="foo" title="*"/>
```
**Output:**
```html
<p>*<img src="foo" title="*"/></p>
```

## Emphasis and strong emphasis

[CM-476] Example 476
**Input:**
```
**<a href="**">
```
**Output:**
```html
<p>**<a href="**"></p>
```

## Emphasis and strong emphasis

[CM-477] Example 477
**Input:**
```
__<a href="__">
```
**Output:**
```html
<p>__<a href="__"></p>
```

## Emphasis and strong emphasis

[CM-478] Example 478
**Input:**
```
*a `*`*
```
**Output:**
```html
<p><em>a <code>*</code></em></p>
```

## Emphasis and strong emphasis

[CM-479] Example 479
**Input:**
```
_a `_`_
```
**Output:**
```html
<p><em>a <code>_</code></em></p>
```

## Emphasis and strong emphasis

[CM-480] Example 480
**Input:**
```
**a<https://foo.bar/?q=**>
```
**Output:**
```html
<p>**a<a href="https://foo.bar/?q=**">https://foo.bar/?q=**</a></p>
```

## Emphasis and strong emphasis

[CM-481] Example 481
**Input:**
```
__a<https://foo.bar/?q=__>
```
**Output:**
```html
<p>__a<a href="https://foo.bar/?q=__">https://foo.bar/?q=__</a></p>
```

## Links

[CM-482] Example 482
**Input:**
```
[link](/uri "title")
```
**Output:**
```html
<p><a href="/uri" title="title">link</a></p>
```

## Links

[CM-483] Example 483
**Input:**
```
[link](/uri)
```
**Output:**
```html
<p><a href="/uri">link</a></p>
```

## Links

[CM-484] Example 484
**Input:**
```
[](./target.md)
```
**Output:**
```html
<p><a href="./target.md"></a></p>
```

## Links

[CM-485] Example 485
**Input:**
```
[link]()
```
**Output:**
```html
<p><a href="">link</a></p>
```

## Links

[CM-486] Example 486
**Input:**
```
[link](<>)
```
**Output:**
```html
<p><a href="">link</a></p>
```

## Links

[CM-487] Example 487
**Input:**
```
[]()
```
**Output:**
```html
<p><a href=""></a></p>
```

## Links

[CM-488] Example 488
**Input:**
```
[link](/my uri)
```
**Output:**
```html
<p>[link](/my uri)</p>
```

## Links

[CM-489] Example 489
**Input:**
```
[link](</my uri>)
```
**Output:**
```html
<p><a href="/my%20uri">link</a></p>
```

## Links

[CM-490] Example 490
**Input:**
```
[link](foo
bar)
```
**Output:**
```html
<p>[link](foo
bar)</p>
```

## Links

[CM-491] Example 491
**Input:**
```
[link](<foo
bar>)
```
**Output:**
```html
<p>[link](<foo
bar>)</p>
```

## Links

[CM-492] Example 492
**Input:**
```
[a](<b)c>)
```
**Output:**
```html
<p><a href="b)c">a</a></p>
```

## Links

[CM-493] Example 493
**Input:**
```
[link](<foo\>)
```
**Output:**
```html
<p>[link](&lt;foo&gt;)</p>
```

## Links

[CM-494] Example 494
**Input:**
```
[a](<b)c
[a](<b)c>
[a](<b>c)
```
**Output:**
```html
<p>[a](&lt;b)c
[a](&lt;b)c&gt;
[a](<b>c)</p>
```

## Links

[CM-495] Example 495
**Input:**
```
[link](\(foo\))
```
**Output:**
```html
<p><a href="(foo)">link</a></p>
```

## Links

[CM-496] Example 496
**Input:**
```
[link](foo(and(bar)))
```
**Output:**
```html
<p><a href="foo(and(bar))">link</a></p>
```

## Links

[CM-497] Example 497
**Input:**
```
[link](foo(and(bar))
```
**Output:**
```html
<p>[link](foo(and(bar))</p>
```

## Links

[CM-498] Example 498
**Input:**
```
[link](foo\(and\(bar\))
```
**Output:**
```html
<p><a href="foo(and(bar)">link</a></p>
```

## Links

[CM-499] Example 499
**Input:**
```
[link](<foo(and(bar)>)
```
**Output:**
```html
<p><a href="foo(and(bar)">link</a></p>
```

## Links

[CM-500] Example 500
**Input:**
```
[link](foo\)\:)
```
**Output:**
```html
<p><a href="foo):">link</a></p>
```

## Links

[CM-501] Example 501
**Input:**
```
[link](#fragment)

[link](https://example.com#fragment)

[link](https://example.com?foo=3#frag)
```
**Output:**
```html
<p><a href="#fragment">link</a></p>
<p><a href="https://example.com#fragment">link</a></p>
<p><a href="https://example.com?foo=3#frag">link</a></p>
```

## Links

[CM-502] Example 502
**Input:**
```
[link](foo\bar)
```
**Output:**
```html
<p><a href="foo%5Cbar">link</a></p>
```

## Links

[CM-503] Example 503
**Input:**
```
[link](foo%20b&auml;)
```
**Output:**
```html
<p><a href="foo%20b%C3%A4">link</a></p>
```

## Links

[CM-504] Example 504
**Input:**
```
[link]("title")
```
**Output:**
```html
<p><a href="%22title%22">link</a></p>
```

## Links

[CM-505] Example 505
**Input:**
```
[link](/url "title")
[link](/url 'title')
[link](/url (title))
```
**Output:**
```html
<p><a href="/url" title="title">link</a>
<a href="/url" title="title">link</a>
<a href="/url" title="title">link</a></p>
```

## Links

[CM-506] Example 506
**Input:**
```
[link](/url "title \"&quot;")
```
**Output:**
```html
<p><a href="/url" title="title &quot;&quot;">link</a></p>
```

## Links

[CM-507] Example 507
**Input:**
```
[link](/url "title")
```
**Output:**
```html
<p><a href="/url%C2%A0%22title%22">link</a></p>
```

## Links

[CM-508] Example 508
**Input:**
```
[link](/url "title "and" title")
```
**Output:**
```html
<p>[link](/url &quot;title &quot;and&quot; title&quot;)</p>
```

## Links

[CM-509] Example 509
**Input:**
```
[link](/url 'title "and" title')
```
**Output:**
```html
<p><a href="/url" title="title &quot;and&quot; title">link</a></p>
```

## Links

[CM-510] Example 510
**Input:**
```
[link](   /uri
  "title"  )
```
**Output:**
```html
<p><a href="/uri" title="title">link</a></p>
```

## Links

[CM-511] Example 511
**Input:**
```
[link] (/uri)
```
**Output:**
```html
<p>[link] (/uri)</p>
```

## Links

[CM-512] Example 512
**Input:**
```
[link [foo [bar]]](/uri)
```
**Output:**
```html
<p><a href="/uri">link [foo [bar]]</a></p>
```

## Links

[CM-513] Example 513
**Input:**
```
[link] bar](/uri)
```
**Output:**
```html
<p>[link] bar](/uri)</p>
```

## Links

[CM-514] Example 514
**Input:**
```
[link [bar](/uri)
```
**Output:**
```html
<p>[link <a href="/uri">bar</a></p>
```

## Links

[CM-515] Example 515
**Input:**
```
[link \[bar](/uri)
```
**Output:**
```html
<p><a href="/uri">link [bar</a></p>
```

## Links

[CM-516] Example 516
**Input:**
```
[link *foo **bar** `#`*](/uri)
```
**Output:**
```html
<p><a href="/uri">link <em>foo <strong>bar</strong> <code>#</code></em></a></p>
```

## Links

[CM-517] Example 517
**Input:**
```
[![moon](moon.jpg)](/uri)
```
**Output:**
```html
<p><a href="/uri"><img src="moon.jpg" alt="moon" /></a></p>
```

## Links

[CM-518] Example 518
**Input:**
```
[foo [bar](/uri)](/uri)
```
**Output:**
```html
<p>[foo <a href="/uri">bar</a>](/uri)</p>
```

## Links

[CM-519] Example 519
**Input:**
```
[foo *[bar [baz](/uri)](/uri)*](/uri)
```
**Output:**
```html
<p>[foo <em>[bar <a href="/uri">baz</a>](/uri)</em>](/uri)</p>
```

## Links

[CM-520] Example 520
**Input:**
```
![[[foo](uri1)](uri2)](uri3)
```
**Output:**
```html
<p><img src="uri3" alt="[foo](uri2)" /></p>
```

## Links

[CM-521] Example 521
**Input:**
```
*[foo*](/uri)
```
**Output:**
```html
<p>*<a href="/uri">foo*</a></p>
```

## Links

[CM-522] Example 522
**Input:**
```
[foo *bar](baz*)
```
**Output:**
```html
<p><a href="baz*">foo *bar</a></p>
```

## Links

[CM-523] Example 523
**Input:**
```
*foo [bar* baz]
```
**Output:**
```html
<p><em>foo [bar</em> baz]</p>
```

## Links

[CM-524] Example 524
**Input:**
```
[foo <bar attr="](baz)">
```
**Output:**
```html
<p>[foo <bar attr="](baz)"></p>
```

## Links

[CM-525] Example 525
**Input:**
```
[foo`](/uri)`
```
**Output:**
```html
<p>[foo<code>](/uri)</code></p>
```

## Links

[CM-526] Example 526
**Input:**
```
[foo<https://example.com/?search=](uri)>
```
**Output:**
```html
<p>[foo<a href="https://example.com/?search=%5D(uri)">https://example.com/?search=](uri)</a></p>
```

## Links

[CM-527] Example 527
**Input:**
```
[foo][bar]

[bar]: /url "title"
```
**Output:**
```html
<p><a href="/url" title="title">foo</a></p>
```

## Links

[CM-528] Example 528
**Input:**
```
[link [foo [bar]]][ref]

[ref]: /uri
```
**Output:**
```html
<p><a href="/uri">link [foo [bar]]</a></p>
```

## Links

[CM-529] Example 529
**Input:**
```
[link \[bar][ref]

[ref]: /uri
```
**Output:**
```html
<p><a href="/uri">link [bar</a></p>
```

## Links

[CM-530] Example 530
**Input:**
```
[link *foo **bar** `#`*][ref]

[ref]: /uri
```
**Output:**
```html
<p><a href="/uri">link <em>foo <strong>bar</strong> <code>#</code></em></a></p>
```

## Links

[CM-531] Example 531
**Input:**
```
[![moon](moon.jpg)][ref]

[ref]: /uri
```
**Output:**
```html
<p><a href="/uri"><img src="moon.jpg" alt="moon" /></a></p>
```

## Links

[CM-532] Example 532
**Input:**
```
[foo [bar](/uri)][ref]

[ref]: /uri
```
**Output:**
```html
<p>[foo <a href="/uri">bar</a>]<a href="/uri">ref</a></p>
```

## Links

[CM-533] Example 533
**Input:**
```
[foo *bar [baz][ref]*][ref]

[ref]: /uri
```
**Output:**
```html
<p>[foo <em>bar <a href="/uri">baz</a></em>]<a href="/uri">ref</a></p>
```

## Links

[CM-534] Example 534
**Input:**
```
*[foo*][ref]

[ref]: /uri
```
**Output:**
```html
<p>*<a href="/uri">foo*</a></p>
```

## Links

[CM-535] Example 535
**Input:**
```
[foo *bar][ref]*

[ref]: /uri
```
**Output:**
```html
<p><a href="/uri">foo *bar</a>*</p>
```

## Links

[CM-536] Example 536
**Input:**
```
[foo <bar attr="][ref]">

[ref]: /uri
```
**Output:**
```html
<p>[foo <bar attr="][ref]"></p>
```

## Links

[CM-537] Example 537
**Input:**
```
[foo`][ref]`

[ref]: /uri
```
**Output:**
```html
<p>[foo<code>][ref]</code></p>
```

## Links

[CM-538] Example 538
**Input:**
```
[foo<https://example.com/?search=][ref]>

[ref]: /uri
```
**Output:**
```html
<p>[foo<a href="https://example.com/?search=%5D%5Bref%5D">https://example.com/?search=][ref]</a></p>
```

## Links

[CM-539] Example 539
**Input:**
```
[foo][BaR]

[bar]: /url "title"
```
**Output:**
```html
<p><a href="/url" title="title">foo</a></p>
```

## Links

[CM-540] Example 540
**Input:**
```
[ẞ]

[SS]: /url
```
**Output:**
```html
<p><a href="/url">ẞ</a></p>
```

## Links

[CM-541] Example 541
**Input:**
```
[Foo
  bar]: /url

[Baz][Foo bar]
```
**Output:**
```html
<p><a href="/url">Baz</a></p>
```

## Links

[CM-542] Example 542
**Input:**
```
[foo] [bar]

[bar]: /url "title"
```
**Output:**
```html
<p>[foo] <a href="/url" title="title">bar</a></p>
```

## Links

[CM-543] Example 543
**Input:**
```
[foo]
[bar]

[bar]: /url "title"
```
**Output:**
```html
<p>[foo]
<a href="/url" title="title">bar</a></p>
```

## Links

[CM-544] Example 544
**Input:**
```
[foo]: /url1

[foo]: /url2

[bar][foo]
```
**Output:**
```html
<p><a href="/url1">bar</a></p>
```

## Links

[CM-545] Example 545
**Input:**
```
[bar][foo\!]

[foo!]: /url
```
**Output:**
```html
<p>[bar][foo!]</p>
```

## Links

[CM-546] Example 546
**Input:**
```
[foo][ref[]

[ref[]: /uri
```
**Output:**
```html
<p>[foo][ref[]</p>
<p>[ref[]: /uri</p>
```

## Links

[CM-547] Example 547
**Input:**
```
[foo][ref[bar]]

[ref[bar]]: /uri
```
**Output:**
```html
<p>[foo][ref[bar]]</p>
<p>[ref[bar]]: /uri</p>
```

## Links

[CM-548] Example 548
**Input:**
```
[[[foo]]]

[[[foo]]]: /url
```
**Output:**
```html
<p>[[[foo]]]</p>
<p>[[[foo]]]: /url</p>
```

## Links

[CM-549] Example 549
**Input:**
```
[foo][ref\[]

[ref\[]: /uri
```
**Output:**
```html
<p><a href="/uri">foo</a></p>
```

## Links

[CM-550] Example 550
**Input:**
```
[bar\\]: /uri

[bar\\]
```
**Output:**
```html
<p><a href="/uri">bar\</a></p>
```

## Links

[CM-551] Example 551
**Input:**
```
[]

[]: /uri
```
**Output:**
```html
<p>[]</p>
<p>[]: /uri</p>
```

## Links

[CM-552] Example 552
**Input:**
```
[
 ]

[
 ]: /uri
```
**Output:**
```html
<p>[
]</p>
<p>[
]: /uri</p>
```

## Links

[CM-553] Example 553
**Input:**
```
[foo][]

[foo]: /url "title"
```
**Output:**
```html
<p><a href="/url" title="title">foo</a></p>
```

## Links

[CM-554] Example 554
**Input:**
```
[*foo* bar][]

[*foo* bar]: /url "title"
```
**Output:**
```html
<p><a href="/url" title="title"><em>foo</em> bar</a></p>
```

## Links

[CM-555] Example 555
**Input:**
```
[Foo][]

[foo]: /url "title"
```
**Output:**
```html
<p><a href="/url" title="title">Foo</a></p>
```

## Links

[CM-556] Example 556
**Input:**
```
[foo] 
[]

[foo]: /url "title"
```
**Output:**
```html
<p><a href="/url" title="title">foo</a>
[]</p>
```

## Links

[CM-557] Example 557
**Input:**
```
[foo]

[foo]: /url "title"
```
**Output:**
```html
<p><a href="/url" title="title">foo</a></p>
```

## Links

[CM-558] Example 558
**Input:**
```
[*foo* bar]

[*foo* bar]: /url "title"
```
**Output:**
```html
<p><a href="/url" title="title"><em>foo</em> bar</a></p>
```

## Links

[CM-559] Example 559
**Input:**
```
[[*foo* bar]]

[*foo* bar]: /url "title"
```
**Output:**
```html
<p>[<a href="/url" title="title"><em>foo</em> bar</a>]</p>
```

## Links

[CM-560] Example 560
**Input:**
```
[[bar [foo]

[foo]: /url
```
**Output:**
```html
<p>[[bar <a href="/url">foo</a></p>
```

## Links

[CM-561] Example 561
**Input:**
```
[Foo]

[foo]: /url "title"
```
**Output:**
```html
<p><a href="/url" title="title">Foo</a></p>
```

## Links

[CM-562] Example 562
**Input:**
```
[foo] bar

[foo]: /url
```
**Output:**
```html
<p><a href="/url">foo</a> bar</p>
```

## Links

[CM-563] Example 563
**Input:**
```
\[foo]

[foo]: /url "title"
```
**Output:**
```html
<p>[foo]</p>
```

## Links

[CM-564] Example 564
**Input:**
```
[foo*]: /url

*[foo*]
```
**Output:**
```html
<p>*<a href="/url">foo*</a></p>
```

## Links

[CM-565] Example 565
**Input:**
```
[foo][bar]

[foo]: /url1
[bar]: /url2
```
**Output:**
```html
<p><a href="/url2">foo</a></p>
```

## Links

[CM-566] Example 566
**Input:**
```
[foo][]

[foo]: /url1
```
**Output:**
```html
<p><a href="/url1">foo</a></p>
```

## Links

[CM-567] Example 567
**Input:**
```
[foo]()

[foo]: /url1
```
**Output:**
```html
<p><a href="">foo</a></p>
```

## Links

[CM-568] Example 568
**Input:**
```
[foo](not a link)

[foo]: /url1
```
**Output:**
```html
<p><a href="/url1">foo</a>(not a link)</p>
```

## Links

[CM-569] Example 569
**Input:**
```
[foo][bar][baz]

[baz]: /url
```
**Output:**
```html
<p>[foo]<a href="/url">bar</a></p>
```

## Links

[CM-570] Example 570
**Input:**
```
[foo][bar][baz]

[baz]: /url1
[bar]: /url2
```
**Output:**
```html
<p><a href="/url2">foo</a><a href="/url1">baz</a></p>
```

## Links

[CM-571] Example 571
**Input:**
```
[foo][bar][baz]

[baz]: /url1
[foo]: /url2
```
**Output:**
```html
<p>[foo]<a href="/url1">bar</a></p>
```

## Images

[CM-572] Example 572
**Input:**
```
![foo](/url "title")
```
**Output:**
```html
<p><img src="/url" alt="foo" title="title" /></p>
```

## Images

[CM-573] Example 573
**Input:**
```
![foo *bar*]

[foo *bar*]: train.jpg "train & tracks"
```
**Output:**
```html
<p><img src="train.jpg" alt="foo bar" title="train &amp; tracks" /></p>
```

## Images

[CM-574] Example 574
**Input:**
```
![foo ![bar](/url)](/url2)
```
**Output:**
```html
<p><img src="/url2" alt="foo bar" /></p>
```

## Images

[CM-575] Example 575
**Input:**
```
![foo [bar](/url)](/url2)
```
**Output:**
```html
<p><img src="/url2" alt="foo bar" /></p>
```

## Images

[CM-576] Example 576
**Input:**
```
![foo *bar*][]

[foo *bar*]: train.jpg "train & tracks"
```
**Output:**
```html
<p><img src="train.jpg" alt="foo bar" title="train &amp; tracks" /></p>
```

## Images

[CM-577] Example 577
**Input:**
```
![foo *bar*][foobar]

[FOOBAR]: train.jpg "train & tracks"
```
**Output:**
```html
<p><img src="train.jpg" alt="foo bar" title="train &amp; tracks" /></p>
```

## Images

[CM-578] Example 578
**Input:**
```
![foo](train.jpg)
```
**Output:**
```html
<p><img src="train.jpg" alt="foo" /></p>
```

## Images

[CM-579] Example 579
**Input:**
```
My ![foo bar](/path/to/train.jpg  "title"   )
```
**Output:**
```html
<p>My <img src="/path/to/train.jpg" alt="foo bar" title="title" /></p>
```

## Images

[CM-580] Example 580
**Input:**
```
![foo](<url>)
```
**Output:**
```html
<p><img src="url" alt="foo" /></p>
```

## Images

[CM-581] Example 581
**Input:**
```
![](/url)
```
**Output:**
```html
<p><img src="/url" alt="" /></p>
```

## Images

[CM-582] Example 582
**Input:**
```
![foo][bar]

[bar]: /url
```
**Output:**
```html
<p><img src="/url" alt="foo" /></p>
```

## Images

[CM-583] Example 583
**Input:**
```
![foo][bar]

[BAR]: /url
```
**Output:**
```html
<p><img src="/url" alt="foo" /></p>
```

## Images

[CM-584] Example 584
**Input:**
```
![foo][]

[foo]: /url "title"
```
**Output:**
```html
<p><img src="/url" alt="foo" title="title" /></p>
```

## Images

[CM-585] Example 585
**Input:**
```
![*foo* bar][]

[*foo* bar]: /url "title"
```
**Output:**
```html
<p><img src="/url" alt="foo bar" title="title" /></p>
```

## Images

[CM-586] Example 586
**Input:**
```
![Foo][]

[foo]: /url "title"
```
**Output:**
```html
<p><img src="/url" alt="Foo" title="title" /></p>
```

## Images

[CM-587] Example 587
**Input:**
```
![foo] 
[]

[foo]: /url "title"
```
**Output:**
```html
<p><img src="/url" alt="foo" title="title" />
[]</p>
```

## Images

[CM-588] Example 588
**Input:**
```
![foo]

[foo]: /url "title"
```
**Output:**
```html
<p><img src="/url" alt="foo" title="title" /></p>
```

## Images

[CM-589] Example 589
**Input:**
```
![*foo* bar]

[*foo* bar]: /url "title"
```
**Output:**
```html
<p><img src="/url" alt="foo bar" title="title" /></p>
```

## Images

[CM-590] Example 590
**Input:**
```
![[foo]]

[[foo]]: /url "title"
```
**Output:**
```html
<p>![[foo]]</p>
<p>[[foo]]: /url &quot;title&quot;</p>
```

## Images

[CM-591] Example 591
**Input:**
```
![Foo]

[foo]: /url "title"
```
**Output:**
```html
<p><img src="/url" alt="Foo" title="title" /></p>
```

## Images

[CM-592] Example 592
**Input:**
```
!\[foo]

[foo]: /url "title"
```
**Output:**
```html
<p>![foo]</p>
```

## Images

[CM-593] Example 593
**Input:**
```
\![foo]

[foo]: /url "title"
```
**Output:**
```html
<p>!<a href="/url" title="title">foo</a></p>
```

## Autolinks

[CM-594] Example 594
**Input:**
```
<http://foo.bar.baz>
```
**Output:**
```html
<p><a href="http://foo.bar.baz">http://foo.bar.baz</a></p>
```

## Autolinks

[CM-595] Example 595
**Input:**
```
<https://foo.bar.baz/test?q=hello&id=22&boolean>
```
**Output:**
```html
<p><a href="https://foo.bar.baz/test?q=hello&amp;id=22&amp;boolean">https://foo.bar.baz/test?q=hello&amp;id=22&amp;boolean</a></p>
```

## Autolinks

[CM-596] Example 596
**Input:**
```
<irc://foo.bar:2233/baz>
```
**Output:**
```html
<p><a href="irc://foo.bar:2233/baz">irc://foo.bar:2233/baz</a></p>
```

## Autolinks

[CM-597] Example 597
**Input:**
```
<MAILTO:FOO@BAR.BAZ>
```
**Output:**
```html
<p><a href="MAILTO:FOO@BAR.BAZ">MAILTO:FOO@BAR.BAZ</a></p>
```

## Autolinks

[CM-598] Example 598
**Input:**
```
<a+b+c:d>
```
**Output:**
```html
<p><a href="a+b+c:d">a+b+c:d</a></p>
```

## Autolinks

[CM-599] Example 599
**Input:**
```
<made-up-scheme://foo,bar>
```
**Output:**
```html
<p><a href="made-up-scheme://foo,bar">made-up-scheme://foo,bar</a></p>
```

## Autolinks

[CM-600] Example 600
**Input:**
```
<https://../>
```
**Output:**
```html
<p><a href="https://../">https://../</a></p>
```

## Autolinks

[CM-601] Example 601
**Input:**
```
<localhost:5001/foo>
```
**Output:**
```html
<p><a href="localhost:5001/foo">localhost:5001/foo</a></p>
```

## Autolinks

[CM-602] Example 602
**Input:**
```
<https://foo.bar/baz bim>
```
**Output:**
```html
<p>&lt;https://foo.bar/baz bim&gt;</p>
```

## Autolinks

[CM-603] Example 603
**Input:**
```
<https://example.com/\[\>
```
**Output:**
```html
<p><a href="https://example.com/%5C%5B%5C">https://example.com/\[\</a></p>
```

## Autolinks

[CM-604] Example 604
**Input:**
```
<foo@bar.example.com>
```
**Output:**
```html
<p><a href="mailto:foo@bar.example.com">foo@bar.example.com</a></p>
```

## Autolinks

[CM-605] Example 605
**Input:**
```
<foo+special@Bar.baz-bar0.com>
```
**Output:**
```html
<p><a href="mailto:foo+special@Bar.baz-bar0.com">foo+special@Bar.baz-bar0.com</a></p>
```

## Autolinks

[CM-606] Example 606
**Input:**
```
<foo\+@bar.example.com>
```
**Output:**
```html
<p>&lt;foo+@bar.example.com&gt;</p>
```

## Autolinks

[CM-607] Example 607
**Input:**
```
<>
```
**Output:**
```html
<p>&lt;&gt;</p>
```

## Autolinks

[CM-608] Example 608
**Input:**
```
< https://foo.bar >
```
**Output:**
```html
<p>&lt; https://foo.bar &gt;</p>
```

## Autolinks

[CM-609] Example 609
**Input:**
```
<m:abc>
```
**Output:**
```html
<p>&lt;m:abc&gt;</p>
```

## Autolinks

[CM-610] Example 610
**Input:**
```
<foo.bar.baz>
```
**Output:**
```html
<p>&lt;foo.bar.baz&gt;</p>
```

## Autolinks

[CM-611] Example 611
**Input:**
```
https://example.com
```
**Output:**
```html
<p>https://example.com</p>
```

## Autolinks

[CM-612] Example 612
**Input:**
```
foo@bar.example.com
```
**Output:**
```html
<p>foo@bar.example.com</p>
```

## Raw HTML

[CM-613] Example 613
**Input:**
```
<a><bab><c2c>
```
**Output:**
```html
<p><a><bab><c2c></p>
```

## Raw HTML

[CM-614] Example 614
**Input:**
```
<a/><b2/>
```
**Output:**
```html
<p><a/><b2/></p>
```

## Raw HTML

[CM-615] Example 615
**Input:**
```
<a  /><b2
data="foo" >
```
**Output:**
```html
<p><a  /><b2
data="foo" ></p>
```

## Raw HTML

[CM-616] Example 616
**Input:**
```
<a foo="bar" bam = 'baz <em>"</em>'
_boolean zoop:33=zoop:33 />
```
**Output:**
```html
<p><a foo="bar" bam = 'baz <em>"</em>'
_boolean zoop:33=zoop:33 /></p>
```

## Raw HTML

[CM-617] Example 617
**Input:**
```
Foo <responsive-image src="foo.jpg" />
```
**Output:**
```html
<p>Foo <responsive-image src="foo.jpg" /></p>
```

## Raw HTML

[CM-618] Example 618
**Input:**
```
<33> <__>
```
**Output:**
```html
<p>&lt;33&gt; &lt;__&gt;</p>
```

## Raw HTML

[CM-619] Example 619
**Input:**
```
<a h*#ref="hi">
```
**Output:**
```html
<p>&lt;a h*#ref=&quot;hi&quot;&gt;</p>
```

## Raw HTML

[CM-620] Example 620
**Input:**
```
<a href="hi'> <a href=hi'>
```
**Output:**
```html
<p>&lt;a href=&quot;hi'&gt; &lt;a href=hi'&gt;</p>
```

## Raw HTML

[CM-621] Example 621
**Input:**
```
< a><
foo><bar/ >
<foo bar=baz
bim!bop />
```
**Output:**
```html
<p>&lt; a&gt;&lt;
foo&gt;&lt;bar/ &gt;
&lt;foo bar=baz
bim!bop /&gt;</p>
```

## Raw HTML

[CM-622] Example 622
**Input:**
```
<a href='bar'title=title>
```
**Output:**
```html
<p>&lt;a href='bar'title=title&gt;</p>
```

## Raw HTML

[CM-623] Example 623
**Input:**
```
</a></foo >
```
**Output:**
```html
<p></a></foo ></p>
```

## Raw HTML

[CM-624] Example 624
**Input:**
```
</a href="foo">
```
**Output:**
```html
<p>&lt;/a href=&quot;foo&quot;&gt;</p>
```

## Raw HTML

[CM-625] Example 625
**Input:**
```
foo <!-- this is a --
comment - with hyphens -->
```
**Output:**
```html
<p>foo <!-- this is a --
comment - with hyphens --></p>
```

## Raw HTML

[CM-626] Example 626
**Input:**
```
foo <!--> foo -->

foo <!---> foo -->
```
**Output:**
```html
<p>foo <!--> foo --&gt;</p>
<p>foo <!---> foo --&gt;</p>
```

## Raw HTML

[CM-627] Example 627
**Input:**
```
foo <?php echo $a; ?>
```
**Output:**
```html
<p>foo <?php echo $a; ?></p>
```

## Raw HTML

[CM-628] Example 628
**Input:**
```
foo <!ELEMENT br EMPTY>
```
**Output:**
```html
<p>foo <!ELEMENT br EMPTY></p>
```

## Raw HTML

[CM-629] Example 629
**Input:**
```
foo <![CDATA[>&<]]>
```
**Output:**
```html
<p>foo <![CDATA[>&<]]></p>
```

## Raw HTML

[CM-630] Example 630
**Input:**
```
foo <a href="&ouml;">
```
**Output:**
```html
<p>foo <a href="&ouml;"></p>
```

## Raw HTML

[CM-631] Example 631
**Input:**
```
foo <a href="\*">
```
**Output:**
```html
<p>foo <a href="\*"></p>
```

## Raw HTML

[CM-632] Example 632
**Input:**
```
<a href="\"">
```
**Output:**
```html
<p>&lt;a href=&quot;&quot;&quot;&gt;</p>
```

## Hard line breaks

[CM-633] Example 633
**Input:**
```
foo  
baz
```
**Output:**
```html
<p>foo<br />
baz</p>
```

## Hard line breaks

[CM-634] Example 634
**Input:**
```
foo\
baz
```
**Output:**
```html
<p>foo<br />
baz</p>
```

## Hard line breaks

[CM-635] Example 635
**Input:**
```
foo       
baz
```
**Output:**
```html
<p>foo<br />
baz</p>
```

## Hard line breaks

[CM-636] Example 636
**Input:**
```
foo  
     bar
```
**Output:**
```html
<p>foo<br />
bar</p>
```

## Hard line breaks

[CM-637] Example 637
**Input:**
```
foo\
     bar
```
**Output:**
```html
<p>foo<br />
bar</p>
```

## Hard line breaks

[CM-638] Example 638
**Input:**
```
*foo  
bar*
```
**Output:**
```html
<p><em>foo<br />
bar</em></p>
```

## Hard line breaks

[CM-639] Example 639
**Input:**
```
*foo\
bar*
```
**Output:**
```html
<p><em>foo<br />
bar</em></p>
```

## Hard line breaks

[CM-640] Example 640
**Input:**
```
`code  
span`
```
**Output:**
```html
<p><code>code   span</code></p>
```

## Hard line breaks

[CM-641] Example 641
**Input:**
```
`code\
span`
```
**Output:**
```html
<p><code>code\ span</code></p>
```

## Hard line breaks

[CM-642] Example 642
**Input:**
```
<a href="foo  
bar">
```
**Output:**
```html
<p><a href="foo  
bar"></p>
```

## Hard line breaks

[CM-643] Example 643
**Input:**
```
<a href="foo\
bar">
```
**Output:**
```html
<p><a href="foo\
bar"></p>
```

## Hard line breaks

[CM-644] Example 644
**Input:**
```
foo\
```
**Output:**
```html
<p>foo\</p>
```

## Hard line breaks

[CM-645] Example 645
**Input:**
```
foo  
```
**Output:**
```html
<p>foo</p>
```

## Hard line breaks

[CM-646] Example 646
**Input:**
```
### foo\
```
**Output:**
```html
<h3>foo\</h3>
```

## Hard line breaks

[CM-647] Example 647
**Input:**
```
### foo  
```
**Output:**
```html
<h3>foo</h3>
```

## Soft line breaks

[CM-648] Example 648
**Input:**
```
foo
baz
```
**Output:**
```html
<p>foo
baz</p>
```

## Soft line breaks

[CM-649] Example 649
**Input:**
```
foo 
 baz
```
**Output:**
```html
<p>foo
baz</p>
```

## Textual content

[CM-650] Example 650
**Input:**
```
hello $.;'there
```
**Output:**
```html
<p>hello $.;'there</p>
```

## Textual content

[CM-651] Example 651
**Input:**
```
Foo χρῆν
```
**Output:**
```html
<p>Foo χρῆν</p>
```

## Textual content

[CM-652] Example 652
**Input:**
```
Multiple     spaces
```
**Output:**
```html
<p>Multiple     spaces</p>
```


Total examples: 652
