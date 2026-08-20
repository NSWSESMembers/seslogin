/**
 * @generated SignedSource<<844803c33d368622b714e663b3a0f430>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type CategoryNewQuery$variables = Record<PropertyKey, never>;
export type CategoryNewQuery$data = {
  readonly nitcGroups: ReadonlyArray<{
    readonly id: string;
    readonly nitcType: string;
  }>;
  readonly ses_participant_types: ReadonlyArray<string>;
};
export type CategoryNewQuery = {
  response: CategoryNewQuery$data;
  variables: CategoryNewQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "alias": null,
    "args": null,
    "concreteType": "NitcGroup",
    "kind": "LinkedField",
    "name": "nitcGroups",
    "plural": true,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "nitcType",
        "storageKey": null
      }
    ],
    "storageKey": null
  },
  {
    "alias": null,
    "args": null,
    "kind": "ScalarField",
    "name": "ses_participant_types",
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [],
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "CategoryNewQuery",
    "selections": (v0/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "CategoryNewQuery",
    "selections": (v0/*: any*/)
  },
  "params": {
    "cacheID": "7b8efb397000547dfcc41630e2a4088a",
    "id": null,
    "metadata": {},
    "name": "CategoryNewQuery",
    "operationKind": "query",
    "text": "query CategoryNewQuery {\n  nitcGroups {\n    id\n    nitcType\n  }\n  ses_participant_types\n}\n"
  }
};
})();

(node as any).hash = "e50ef31c6121fed407a164fde530df57";

export default node;
