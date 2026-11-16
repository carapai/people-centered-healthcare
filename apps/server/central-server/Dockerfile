# Central's OCI image (deployment.md § Releases): the release job's central-server binary on a
# distroless glibc base, run as a non-root user. The SurrealDB server, secrets and the blob and
# backup volumes come from the deployment (README.md § Configuration).
# Build from a directory holding the binary: docker build -f Dockerfile <dir>
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:cb87acc574abe719b6e5c547384defa2f7b1f24af9605d3f618cf852495d8be2
COPY --chmod=0755 central-server /usr/local/bin/central-server
USER nonroot
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/central-server"]
