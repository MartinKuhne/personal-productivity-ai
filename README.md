# Personal productivity AI

## Goal

Increase my personal productivity with AI without compromising security or privacy

## Vision

A library of markdown formatted notes as a rich knowledge base to then enable everyday integration with email, calendar, contacts, todo lists and web search.
Examples: Create tasks for importent e-mail reminders. Research the maintenance schedule for my car.  

## Solution approach

The solution centers around a *markdown document library*. The more information this library contains about you and your history, the more useful AI as a tool is going to be. Information gets into the library by either writing it, or by _distilling_ it from other sources. Everything builds on everything else to provide a rich context.

This is an as-simple-as possible design. There is no RAG, not vector databases, no magical MCP server. The magic is that every interaction, research or data collection becomes a note to be used for future reference. 

All of this in a fast, single binary, with beautifully rendered markdown.

![Screenshot](doc/img/Screenshot.png)

## Tools

The system has access to
- Contacts (via DAV)
- Calendar (via DAV)
- E-mail (via JMAP)
- Web fetch
- Web search (with additional configuration)
- Weather
- Local markdown file grep and edits

There is no bash / no system access.

## Use cases

- Find trends in my electricity use
- Find places to hike or go to where the weather is nice
- Find all the items ever bought from some retailer
- Create a summary document of your medical history
- Investigate today's job alerts sent via e-mail and compare against my resume

## What it's not

I don't mind writing _raw_ markdown, and I didn't like the WYSIWYG editing experience a certain famous product provides. So this doesn't have any of this, there is a simple text editor, or you can launch any external tool.

It does not concern itself how you access or run an LLM. AI functions need an OpenAI compatible endpoint.

## Notes

I didn't start out as a rust developer. I use this daily, and it's also a research project to dive into a different ecosystem, and to manage a large codebase with 0% human hands-on coding. Rust is great! You are in good hands.
