/**
 * @generated SignedSource<<224ab985a05ff36fb1c4cca17486dff3>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type CategoryListDisableMutation$variables = {
  id: string;
  isVirtual: boolean;
  name: string;
  nitcGroupId?: string | null | undefined;
  nitcParticipantType?: string | null | undefined;
};
export type CategoryListDisableMutation$data = {
  readonly updateCategory: {
    readonly enabled: boolean;
    readonly id: string;
    readonly name: string;
  };
};
export type CategoryListDisableMutation = {
  response: CategoryListDisableMutation$data;
  variables: CategoryListDisableMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "isVirtual"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "name"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "nitcGroupId"
},
v4 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "nitcParticipantType"
},
v5 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Literal",
        "name": "enabled",
        "value": false
      },
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "isVirtual",
        "variableName": "isVirtual"
      },
      {
        "kind": "Variable",
        "name": "name",
        "variableName": "name"
      },
      {
        "kind": "Variable",
        "name": "nitcGroupId",
        "variableName": "nitcGroupId"
      },
      {
        "kind": "Variable",
        "name": "nitcParticipantType",
        "variableName": "nitcParticipantType"
      }
    ],
    "concreteType": "Category",
    "kind": "LinkedField",
    "name": "updateCategory",
    "plural": false,
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
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "enabled",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*: any*/),
      (v1/*: any*/),
      (v2/*: any*/),
      (v3/*: any*/),
      (v4/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "CategoryListDisableMutation",
    "selections": (v5/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v0/*: any*/),
      (v2/*: any*/),
      (v1/*: any*/),
      (v3/*: any*/),
      (v4/*: any*/)
    ],
    "kind": "Operation",
    "name": "CategoryListDisableMutation",
    "selections": (v5/*: any*/)
  },
  "params": {
    "cacheID": "e9c03c330c7c42ebe9823ab298340351",
    "id": null,
    "metadata": {},
    "name": "CategoryListDisableMutation",
    "operationKind": "mutation",
    "text": "mutation CategoryListDisableMutation(\n  $id: ID!\n  $name: String!\n  $isVirtual: Boolean!\n  $nitcGroupId: String\n  $nitcParticipantType: String\n) {\n  updateCategory(id: $id, name: $name, enabled: false, isVirtual: $isVirtual, nitcGroupId: $nitcGroupId, nitcParticipantType: $nitcParticipantType) {\n    id\n    name\n    enabled\n  }\n}\n"
  }
};
})();

(node as any).hash = "a574248c7ecc5b3533596bbaf084c5d1";

export default node;
