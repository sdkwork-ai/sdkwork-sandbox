export interface PageInfo {
  mode: 'cursor';
  /** Effective page size for this response */
  pageSize?: number;
  /** Opaque next-page token; a page that ends the enumeration omits it */
  nextCursor?: string | null;
  hasMore?: boolean;
}
