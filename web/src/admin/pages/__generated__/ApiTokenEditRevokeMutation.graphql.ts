/**
 * @generated SignedSource<<706f1ad1b7f48c84c124b080fa713dc3>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ApiTokenEditRevokeMutation$variables = {
  id: string;
};
export type ApiTokenEditRevokeMutation$data = {
  readonly revokeApiToken: boolean;
};
export type ApiTokenEditRevokeMutation = {
  response: ApiTokenEditRevokeMutation$data;
  variables: ApiTokenEditRevokeMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      }
    ],
    "kind": "ScalarField",
    "name": "revokeApiToken",
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "ApiTokenEditRevokeMutation",
    "selections": (v1/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "ApiTokenEditRevokeMutation",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "8f7517652215bb681f394d4c3fe9ed31",
    "id": null,
    "metadata": {},
    "name": "ApiTokenEditRevokeMutation",
    "operationKind": "mutation",
    "text": "mutation ApiTokenEditRevokeMutation(\n  $id: ID!\n) {\n  revokeApiToken(id: $id)\n}\n"
  }
};
})();

(node as any).hash = "d0901f926d1f4638b26b353f2523dbf4";

export default node;
