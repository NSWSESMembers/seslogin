/**
 * @generated SignedSource<<9b5f5f6cd30384438ea441f5de867b88>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type NitcGroupNewQuery$variables = Record<PropertyKey, never>;
export type NitcGroupNewQuery$data = {
  readonly ses_nonincident_tags: ReadonlyArray<{
    readonly id: string;
    readonly name: string;
  }>;
  readonly ses_nonincident_types: ReadonlyArray<string>;
};
export type NitcGroupNewQuery = {
  response: NitcGroupNewQuery$data;
  variables: NitcGroupNewQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "alias": null,
    "args": null,
    "kind": "ScalarField",
    "name": "ses_nonincident_types",
    "storageKey": null
  },
  {
    "alias": null,
    "args": null,
    "concreteType": "SesNonIncidentTag",
    "kind": "LinkedField",
    "name": "ses_nonincident_tags",
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
        "name": "name",
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
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "NitcGroupNewQuery",
    "selections": (v0/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "NitcGroupNewQuery",
    "selections": (v0/*: any*/)
  },
  "params": {
    "cacheID": "edc7acb79e25012ae7bd5650c4b8730d",
    "id": null,
    "metadata": {},
    "name": "NitcGroupNewQuery",
    "operationKind": "query",
    "text": "query NitcGroupNewQuery {\n  ses_nonincident_types\n  ses_nonincident_tags {\n    id\n    name\n  }\n}\n"
  }
};
})();

(node as any).hash = "161b77eba1fa2f44ba9b788cc1acbd5f";

export default node;
