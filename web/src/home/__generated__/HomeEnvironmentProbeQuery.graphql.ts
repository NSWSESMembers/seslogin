/**
 * @generated SignedSource<<b472f53f8c01e922bf1c12e677a5e754>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type HomeEnvironmentProbeQuery$variables = Record<PropertyKey, never>;
export type HomeEnvironmentProbeQuery$data = {
  readonly environment: {
    readonly gitRev: string;
    readonly isProdDb: boolean;
  };
};
export type HomeEnvironmentProbeQuery = {
  response: HomeEnvironmentProbeQuery$data;
  variables: HomeEnvironmentProbeQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "alias": null,
    "args": null,
    "concreteType": "Environment",
    "kind": "LinkedField",
    "name": "environment",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "gitRev",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "isProdDb",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [],
    "kind": "Fragment",
    "metadata": null,
    "name": "HomeEnvironmentProbeQuery",
    "selections": (v0/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "HomeEnvironmentProbeQuery",
    "selections": (v0/*: any*/)
  },
  "params": {
    "cacheID": "834abf15635611495e6ef9e47a400c5d",
    "id": null,
    "metadata": {},
    "name": "HomeEnvironmentProbeQuery",
    "operationKind": "query",
    "text": "query HomeEnvironmentProbeQuery {\n  environment {\n    gitRev\n    isProdDb\n  }\n}\n"
  }
};
})();

(node as any).hash = "82478ba567a78f11bc8344859986e1ac";

export default node;
