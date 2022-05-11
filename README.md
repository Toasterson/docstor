# docstor
Document storage service so a document can never be deleted. Only worked on. Uses rocksdb to safe the documents in local storage.

# Components
- **libdocapi** GRpc types and proto files
- **docstord** Daemon and service that saves the documents, tags and other data needed for operations on the documents
- **docstoradm** Admin Utility to make backups start/stop the daemon and call restoeres. Must be run on server machine
- **docstor** CLI client to interact with the server.
