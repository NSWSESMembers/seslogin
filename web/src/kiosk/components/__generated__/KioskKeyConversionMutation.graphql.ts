/**
 * @generated SignedSource<<de99b8791421e8460aef61f7ec6eab9d>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type KioskKeyConversionMutation$variables = {
  proof: string;
  publicKey: string;
};
export type KioskKeyConversionMutation$data = {
  readonly convertSessionToKey: {
    readonly id: string;
  };
};
export type KioskKeyConversionMutation = {
  response: KioskKeyConversionMutation$data;
  variables: KioskKeyConversionMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "proof"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "publicKey"
},
v2 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "proof",
        "variableName": "proof"
      },
      {
        "kind": "Variable",
        "name": "publicKey",
        "variableName": "publicKey"
      }
    ],
    "concreteType": "Session",
    "kind": "LinkedField",
    "name": "convertSessionToKey",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "KioskKeyConversionMutation",
    "selections": (v2/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*:: as any*/),
      (v0/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "KioskKeyConversionMutation",
    "selections": (v2/*:: as any*/)
  },
  "params": {
    "cacheID": "7a70242d20364bf21a6cad87f2abb126",
    "id": null,
    "metadata": {},
    "name": "KioskKeyConversionMutation",
    "operationKind": "mutation",
    "text": "mutation KioskKeyConversionMutation(\n  $publicKey: String!\n  $proof: String!\n) {\n  convertSessionToKey(publicKey: $publicKey, proof: $proof) {\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "d62406ab1aded6081bb1f93bbc8d3683";

export default node;
