/**
 * @generated SignedSource<<b9709ea6a801073226e7a47112a089ae>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SessionEnrollKiosksQuery$variables = {
  location: string;
};
export type SessionEnrollKiosksQuery$data = {
  readonly location: {
    readonly sessions: ReadonlyArray<{
      readonly id: string;
      readonly keyEnrolled: boolean;
      readonly keyReleasedAt: number | null | undefined;
      readonly lastContact: number | null | undefined;
      readonly name: string;
      readonly version: number;
    }>;
    readonly viewerCanEdit: boolean;
  };
};
export type SessionEnrollKiosksQuery = {
  response: SessionEnrollKiosksQuery$data;
  variables: SessionEnrollKiosksQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "location"
  }
],
v1 = [
  {
    "kind": "Variable",
    "name": "id",
    "variableName": "location"
  }
],
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "viewerCanEdit",
  "storageKey": null
},
v3 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v4 = {
  "alias": null,
  "args": null,
  "concreteType": "Session",
  "kind": "LinkedField",
  "name": "sessions",
  "plural": true,
  "selections": [
    (v3/*:: as any*/),
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "name",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "lastContact",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "version",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "keyEnrolled",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "keyReleasedAt",
      "storageKey": null
    }
  ],
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "SessionEnrollKiosksQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*:: as any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v2/*:: as any*/),
          (v4/*:: as any*/)
        ],
        "storageKey": null
      }
    ],
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "SessionEnrollKiosksQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*:: as any*/),
        "concreteType": "Location",
        "kind": "LinkedField",
        "name": "location",
        "plural": false,
        "selections": [
          (v2/*:: as any*/),
          (v4/*:: as any*/),
          (v3/*:: as any*/)
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "743f06ecf48527204e5790c86ecd313c",
    "id": null,
    "metadata": {},
    "name": "SessionEnrollKiosksQuery",
    "operationKind": "query",
    "text": "query SessionEnrollKiosksQuery(\n  $location: ID!\n) {\n  location(id: $location) {\n    viewerCanEdit\n    sessions {\n      id\n      name\n      lastContact\n      version\n      keyEnrolled\n      keyReleasedAt\n    }\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "19f0e1bbed36cf6824741ebf2bfbd3ad";

export default node;
