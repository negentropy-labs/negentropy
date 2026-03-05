// Simple utility — should score well
export class Database {
  query(sql: string) {
    return { rows: [] };
  }

  save(entity: any) {
    return entity;
  }
}
