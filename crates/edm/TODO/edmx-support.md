# edmx support

the parser currently supports only "edm:schema" and it's children.
we need a parser that can deal with complete edmx files including handling references to other files and multiple schemas.
ideally this is a different parser function since the ability to parse XML documents with the root `<Schema`. is very usefull and a building block for the EDMX parser