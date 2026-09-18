// relay-test-utils ships no types of its own (and the @types/relay-test-utils package
// on npm only covers older Relay major versions), so this declares the minimal surface
// this codebase actually calls, matching the .js.flow sources under
// node_modules/relay-test-utils for v20.1.1.
declare module "relay-test-utils" {
  import type {
    Environment,
    GraphQLSingularResponse,
    OperationDescriptor,
  } from "relay-runtime";

  type MockResolverContext = {
    parentType: string | null | undefined;
    name: string | null | undefined;
    alias: string | null | undefined;
    path: ReadonlyArray<string> | null | undefined;
    args: Record<string, unknown> | null | undefined;
  };
  type MockResolver = (
    context: MockResolverContext,
    generateId: () => number,
  ) => unknown;
  type MockResolvers = Record<string, MockResolver>;

  interface MockFunctions {
    resolve(
      request: OperationDescriptor,
      payload: ReadonlyArray<GraphQLSingularResponse> | GraphQLSingularResponse,
    ): void;
    getAllOperations(): ReadonlyArray<OperationDescriptor>;
    getMostRecentOperation(): OperationDescriptor;
    resolveMostRecentOperation(
      payload:
        | GraphQLSingularResponse
        | ((operation: OperationDescriptor) => GraphQLSingularResponse),
    ): void;
  }

  export interface RelayMockEnvironment extends Environment {
    readonly mock: MockFunctions;
  }

  export function createMockEnvironment(): RelayMockEnvironment;

  export const MockPayloadGenerator: {
    generate(
      operation: OperationDescriptor,
      mockResolvers?: MockResolvers | null,
    ): GraphQLSingularResponse;
  };
}
